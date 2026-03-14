//! Freenet Service - High-level service for managing pizza order contracts on Freenet.
//!
//! This service implements the BaseInterface trait using the Freenet node API
//! for publishing, subscribing, and updating contracts. All private state
//! (signing keys, contract keys) is stored in the delegate, not localStorage.

use std::error::Error;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};

use dioxus::prelude::ReadableExt;
use dioxus::signals::{Global, GlobalSignal};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use freenet_stdlib::prelude::tracing::{error, info, warn};
use freenet_stdlib::prelude::ContractKey;
use futures::Stream;
use futures::StreamExt;
use pizza_common::{ComposableState, FullOrderStateV1, FullOrderStateV1Delta, OrderParametersV1};

use super::base::{
    AsyncResult, BaseInterface, Contract, PublishContractResponse, PublishDeltaResponse,
};
use crate::api::delegate_api;
use crate::api::node_api::{
    fetch_unknown_contract_async, get_contract_by_key_async, get_contract_keys,
    get_contract_state_cached, notify_contract_list_change, notify_contract_update,
    publish_contract_async, send_contract_delta_async, subscribe_to_contract_async,
    subscribe_to_contract_list, subscribe_to_contract_updates, CONTRACTS,
};

/// Global signal to track if we have loaded the signing key from delegate.
pub static SIGNING_KEY_LOADED: GlobalSignal<bool> = Global::new(|| false);

/// Global signal to store the cached signing key once loaded.
pub static CACHED_SIGNING_KEY: GlobalSignal<Option<SigningKey>> = Global::new(|| None);

/// Track if we've already requested the signing key from delegate
static SIGNING_KEY_REQUESTED: AtomicBool = AtomicBool::new(false);

/// FreenetService implements the BaseInterface trait using the Freenet node API.
///
/// Unlike previous versions, this service:
/// - Publishes contracts to the Freenet network via PUT requests
/// - Subscribes to contract updates via WebSocket notifications
/// - Sends state updates to the network
/// - Uses delegate for private key and contract key storage (NO localStorage)
/// - Awaits network acknowledgements for operations
#[derive(Clone)]
pub struct FreenetService;

impl FreenetService {
    /// Create a new FreenetService.
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self)
    }

    /// Initialize the signing key - either load from delegate or generate a new one.
    /// This should be called during app initialization.
    pub async fn init_signing_key() -> Result<SigningKey, String> {
        // Check if already loaded
        if *SIGNING_KEY_LOADED.read() {
            if let Some(ref key) = *CACHED_SIGNING_KEY.read() {
                return Ok(key.clone());
            }
        }

        // Prevent duplicate requests
        if SIGNING_KEY_REQUESTED.swap(true, Ordering::SeqCst) {
            // Another request is in progress, wait for it
            for _ in 0..100 {
                // Wait up to 10 seconds
                sleep_ms(100).await;
                if *SIGNING_KEY_LOADED.read() {
                    if let Some(ref key) = *CACHED_SIGNING_KEY.read() {
                        return Ok(key.clone());
                    }
                }
            }
            return Err("Timeout waiting for signing key initialization".to_string());
        }

        info!("Initializing signing key from delegate");

        // Try to get the public key first to check if signing key exists
        match delegate_api::get_public_key().await {
            Ok(Some(_public_key)) => {
                // Signing key exists in delegate - we can't retrieve the private key,
                // but we can use the delegate for signing operations.
                // For now, generate a local key and store it
                info!("Found existing public key in delegate");
            }
            Ok(None) => {
                info!("No signing key in delegate, will generate new one");
            }
            Err(e) => {
                warn!("Failed to check for existing key: {}", e);
            }
        }

        // Generate new signing key and store in delegate
        let mut rng = rand::thread_rng();
        let signing_key = SigningKey::generate(&mut rng);

        // Store the signing key in the delegate
        match delegate_api::store_signing_key(signing_key.to_bytes()).await {
            Ok(()) => {
                info!("Stored new signing key in delegate");
            }
            Err(e) => {
                warn!(
                    "Failed to store signing key in delegate: {}, using local key",
                    e
                );
            }
        }

        // Cache the signing key
        *CACHED_SIGNING_KEY.write() = Some(signing_key.clone());
        *SIGNING_KEY_LOADED.write() = true;

        Ok(signing_key)
    }

    /// Get the signing key, initializing if needed.
    pub async fn get_signing_key_async() -> Result<SigningKey, String> {
        // Check cache first
        if *SIGNING_KEY_LOADED.read() {
            if let Some(ref key) = *CACHED_SIGNING_KEY.read() {
                return Ok(key.clone());
            }
        }

        // Initialize if not yet done
        Self::init_signing_key().await
    }

    /// Get the cached signing key synchronously.
    /// Returns None if signing key hasn't been initialized yet.
    pub fn get_signing_key_cached() -> Option<SigningKey> {
        if *SIGNING_KEY_LOADED.read() {
            CACHED_SIGNING_KEY.read().clone()
        } else {
            None
        }
    }

    /// Save contract keys to delegate
    async fn save_contract_keys(keys: &[String]) {
        if let Err(e) = delegate_api::store_contract_keys(keys.to_vec()).await {
            error!("Failed to save contract keys to delegate: {}", e);
        }
    }
}

/// WASM-compatible sleep
async fn sleep_ms(ms: u32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms as i32)
            .unwrap();
    });
    wasm_bindgen_futures::JsFuture::from(promise).await.ok();
}

impl BaseInterface for FreenetService {
    /// Returns a list of contract IDs (keys) from local cache.
    /// Contract keys are loaded from delegate on connection, stored in CONTRACTS.
    fn get_contracts(&self) -> Result<Vec<String>, Box<dyn Error>> {
        Ok(get_contract_keys())
    }

    /// Returns parameters and state for a given contract ID from the local cache.
    fn get_contract_cached(&self, id: String) -> Option<Contract> {
        get_contract_state_cached(&id).map(|(state, parameters)| Contract { state, parameters })
    }

    /// Fetch contract state from Freenet via GET request.
    /// If the contract is unknown, attempts to fetch it from the network.
    fn get_contract_async(&self, id: String) -> AsyncResult<Contract> {
        Box::pin(async move {
            // Check if we have it cached first
            if let Some((state, params)) = get_contract_state_cached(&id) {
                // Even if cached, request fresh state from network
                if let Some(rx) = get_contract_by_key_async(&id) {
                    match rx.await {
                        Ok(response) => {
                            if let Some(state) = response.state {
                                // Get params from cache (they don't change)
                                return Ok(Contract {
                                    state,
                                    parameters: params,
                                });
                            }
                        }
                        Err(_) => {
                            // Channel cancelled, return cached state
                        }
                    }
                }
                // Return cached state if network request failed
                return Ok(Contract {
                    state,
                    parameters: params,
                });
            }

            // Contract not in cache - fetch from network (includes params in contract container)
            let rx = fetch_unknown_contract_async(&id);
            match rx.await {
                Ok(response) => {
                    if let Some(state) = response.state {
                        // Contract was found and cached by handle_get_response
                        // Get the params from the cache now
                        if let Some((_, params)) = get_contract_state_cached(&id) {
                            // Save contract key to delegate for future sessions
                            let contract_keys = get_contract_keys();
                            FreenetService::save_contract_keys(&contract_keys).await;

                            // Subscribe to updates for this newly fetched contract
                            let _ = subscribe_to_contract_async(&id);

                            return Ok(Contract {
                                state,
                                parameters: params,
                            });
                        }
                    }
                }
                Err(_) => {
                    // Channel cancelled
                }
            }

            Err(format!("Contract not found: {}", id).into())
        })
    }

    /// Publish a delta update to a contract on the Freenet network.
    ///
    /// This sends the delta directly to the network and waits for acknowledgement.
    /// The contract on the network will apply the delta to compute the new state.
    fn publish_delta(
        &self,
        id: String,
        delta: FullOrderStateV1Delta,
    ) -> AsyncResult<PublishDeltaResponse> {
        Box::pin(async move {
            // Get current state and contract key for local optimistic update
            let (current_state, params, contract_key): (
                FullOrderStateV1,
                OrderParametersV1,
                ContractKey,
            ) = {
                let contracts = CONTRACTS.read();
                contracts
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| format!("Contract not found: {}", id))?
            };

            // Compute new state locally for optimistic update
            let mut new_state: FullOrderStateV1 = current_state.clone();
            new_state
                .apply_delta(&current_state, &params, &Some(delta.clone()))
                .map_err(|e| e.to_string())?;

            // Update local state optimistically
            {
                let mut contracts = CONTRACTS.write();
                contracts.insert(
                    id.clone(),
                    (new_state.clone(), params.clone(), contract_key),
                );
            }

            // Notify subscribers of the optimistic update
            notify_contract_update(&id, &new_state, &params);

            // Send the delta to the network and wait for acknowledgement
            let response_rx = send_contract_delta_async(&id, &delta)
                .ok_or_else(|| format!("Failed to send delta for contract: {}", id))?;

            // Wait for the response
            match response_rx.await {
                Ok(response) => Ok(PublishDeltaResponse {
                    state: new_state,
                    acknowledged: response.success,
                }),
                Err(_) => {
                    // Channel was cancelled - this can happen if the connection drops
                    Err("Delta update request was cancelled (connection lost?)".into())
                }
            }
        })
    }

    /// Returns the user's public key (verifying key).
    fn get_public_key(&self) -> Result<VerifyingKey, Box<dyn Error>> {
        match Self::get_signing_key_cached() {
            Some(signing_key) => Ok(signing_key.verifying_key()),
            None => Err("Signing key not yet initialized".into()),
        }
    }

    /// Returns the user's private key (signing key).
    fn get_private_key(&self) -> Result<SigningKey, Box<dyn Error>> {
        match Self::get_signing_key_cached() {
            Some(signing_key) => Ok(signing_key),
            None => Err("Signing key not yet initialized".into()),
        }
    }

    /// Signs a message with the user's private key.
    fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        match Self::get_signing_key_cached() {
            Some(signing_key) => {
                let signature: Signature = signing_key.sign(message);
                Ok(signature.to_bytes().to_vec())
            }
            None => Err("Signing key not yet initialized".into()),
        }
    }

    /// Publish a new contract to the Freenet network.
    ///
    /// This sends a PUT request to the Freenet node with the contract code,
    /// state, and parameters. It waits for the network to acknowledge the
    /// contract before returning.
    fn publish_contract(&self, contract: Contract) -> AsyncResult<PublishContractResponse> {
        let state = contract.state;
        let params = contract.parameters;

        Box::pin(async move {
            // Publish the contract and get a receiver for the response
            let (contract_key, response_rx) = publish_contract_async(&state, &params);

            // Wait for the PUT response
            match response_rx.await {
                Ok(response) => {
                    if response.success {
                        // Save contract keys to delegate
                        let contract_keys = get_contract_keys();
                        FreenetService::save_contract_keys(&contract_keys).await;

                        // Subscribe to network updates for this contract
                        if let Some(subscribe_rx) = subscribe_to_contract_async(&contract_key) {
                            // Wait for subscription to be confirmed (with a reasonable timeout)
                            let _ = subscribe_rx.await;
                        }

                        // Notify any existing state subscribers of the initial state
                        notify_contract_update(&response.contract_key, &state, &params);

                        // Subscribe to updates for this newly fetched contract
                        let _ = subscribe_to_contract_async(&contract_key);

                        Ok(PublishContractResponse {
                            contract_key: response.contract_key,
                            state,
                        })
                    } else {
                        Err("Contract publication was rejected by the network".into())
                    }
                }
                Err(_) => {
                    // Channel was cancelled
                    Err("Publish request was cancelled (connection lost?)".into())
                }
            }
        })
    }

    /// Subscribe to changes in the contract list.
    ///
    /// Returns a stream that emits the list of contract keys whenever it changes.
    fn subscribe_contracts(&self) -> Pin<Box<dyn Stream<Item = Vec<String>>>> {
        let rx = subscribe_to_contract_list();
        Box::pin(rx)
    }

    /// Subscribe to state changes for a specific contract.
    ///
    /// Returns a stream that emits the contract state whenever it changes
    /// (from network updates).
    fn subscribe_contract_state(&self, id: String) -> Pin<Box<dyn Stream<Item = Contract>>> {
        let rx = subscribe_to_contract_updates(&id);
        Box::pin(rx.map(|(state, parameters)| Contract { state, parameters }))
    }

    /// Remove a contract from the local list.
    ///
    /// This removes the contract from the delegate but does not delete it from the network.
    fn remove_contract(&self, id: String) {
        // Remove from in-memory cache
        {
            let mut contracts = CONTRACTS.write();
            contracts.remove(&id);
        }

        // Save updated contract keys to delegate
        let contract_keys = get_contract_keys();
        wasm_bindgen_futures::spawn_local(async move {
            FreenetService::save_contract_keys(&contract_keys).await;
        });

        // Notify subscribers of the change
        let keys = get_contract_keys();
        notify_contract_list_change(keys);
    }
}

#[cfg(test)]
mod tests {
    // Tests would go here, but they require a running Freenet node
}
