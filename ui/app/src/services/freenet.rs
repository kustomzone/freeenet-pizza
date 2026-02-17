//! Freenet Service - High-level service for managing pizza order contracts on Freenet.
//!
//! This service implements the BaseInterface trait using the Freenet node API
//! for publishing, subscribing, and updating contracts.

use std::error::Error;
use std::pin::Pin;

use dioxus::prelude::ReadableExt;
use dioxus::signals::Writable;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use freenet_stdlib::prelude::ContractKey;
use futures::Stream;
use futures::StreamExt;
use pizza_common::{ComposableState, FullOrderStateV1, FullOrderStateV1Delta, OrderParametersV1};
use web_sys::Storage;

use super::base::{
    AsyncResult, BaseInterface, Contract, PublishContractResponse, PublishDeltaResponse,
};
use crate::api::node_api::{
    get_contract_keys, get_contract_state_cached, get_contract_by_key_async,
    get_contract_state_async, fetch_unknown_contract_async, notify_contract_update,
    publish_contract_async, send_contract_delta_async, subscribe_to_contract_async,
    subscribe_to_contract_list, subscribe_to_contract_updates, CONTRACTS,
};

/// Private key storage key in browser storage (for key persistence across sessions)
const PRIVATE_KEY_KEY: &str = "pizza_private_key";
/// Contract list storage key (stores list of contract key strings)
const CONTRACT_LIST_KEY: &str = "pizza_contract_keys";
/// Contract params storage key prefix (stores serialized params for each contract)
const CONTRACT_PARAMS_PREFIX: &str = "pizza_contract_params_";

/// FreenetService implements the BaseInterface trait using the Freenet node API.
///
/// Unlike LocalStorageService, this service:
/// - Publishes contracts to the Freenet network via PUT requests
/// - Subscribes to contract updates via WebSocket notifications
/// - Sends state updates to the network
/// - Uses browser storage for private key and contract key list persistence
/// - Awaits network acknowledgements for operations
#[derive(Clone)]
pub struct FreenetService {
    /// The user's signing key (persisted in browser storage)
    signing_key: SigningKey,
    /// Browser localStorage handle
    storage: Storage,
}

impl FreenetService {
    /// Create a new FreenetService.
    ///
    /// This will load the signing key from browser storage or generate a new one.
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let window = web_sys::window().ok_or("no window")?;
        let storage = window
            .local_storage()
            .map_err(|e| format!("{:?}", e))?
            .ok_or("no local storage")?;

        let signing_key = Self::load_or_create_signing_key(&storage)?;

        Ok(Self { signing_key, storage })
    }

    /// Load the signing key from browser storage or create a new one.
    fn load_or_create_signing_key(storage: &Storage) -> Result<SigningKey, Box<dyn Error>> {
        match storage
            .get_item(PRIVATE_KEY_KEY)
            .map_err(|e| format!("{:?}", e))?
        {
            Some(hex_key) => {
                let bytes = hex::decode(hex_key)?;
                let bytes: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| "invalid private key length")?;
                Ok(SigningKey::from_bytes(&bytes))
            }
            None => {
                let mut rng = rand::thread_rng();
                let signing_key = SigningKey::generate(&mut rng);
                let hex_key = hex::encode(signing_key.to_bytes());
                storage
                    .set_item(PRIVATE_KEY_KEY, &hex_key)
                    .map_err(|e| format!("{:?}", e))?;
                Ok(signing_key)
            }
        }
    }

    /// Load contract keys from localStorage.
    fn load_contract_keys(&self) -> Vec<String> {
        match self.storage.get_item(CONTRACT_LIST_KEY) {
            Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

}

/// Static helper to save a contract key and its params to localStorage (for use in async blocks).
fn save_contract_key_static(storage: &Storage, key: &str, params: &OrderParametersV1) {
    // Load current list
    let mut contract_keys: Vec<String> = match storage.get_item(CONTRACT_LIST_KEY) {
        Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_default(),
        _ => Vec::new(),
    };

    // Add new key if not present
    if !contract_keys.contains(&key.to_string()) {
        contract_keys.push(key.to_string());
        if let Ok(json) = serde_json::to_string(&contract_keys) {
            let _ = storage.set_item(CONTRACT_LIST_KEY, &json);
        }
    }

    // Save params for this contract
    let params_key = format!("{}{}", CONTRACT_PARAMS_PREFIX, key);
    if let Ok(params_json) = serde_json::to_string(params) {
        let _ = storage.set_item(&params_key, &params_json);
    }
}

/// Load contract params from localStorage.
fn load_contract_params_static(storage: &Storage, key: &str) -> Option<OrderParametersV1> {
    let params_key = format!("{}{}", CONTRACT_PARAMS_PREFIX, key);
    match storage.get_item(&params_key) {
        Ok(Some(json)) => serde_json::from_str(&json).ok(),
        _ => None,
    }
}

impl BaseInterface for FreenetService {
    /// Returns a list of contract IDs (keys) from localStorage and local cache.
    fn get_contracts(&self) -> Result<Vec<String>, Box<dyn Error>> {
        // Merge keys from localStorage with in-memory cache
        let mut keys = self.load_contract_keys();
        for key in get_contract_keys() {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        Ok(keys)
    }

    /// Returns parameters and state for a given contract ID from the local cache.
    fn get_contract_cached(&self, id: String) -> Option<Contract> {
        get_contract_state_cached(&id).map(|(state, parameters)| Contract { state, parameters })
    }

    /// Fetch contract state from Freenet via GET request.
    /// If the contract is unknown, attempts to fetch it from the network.
    fn get_contract_async(&self, id: String) -> AsyncResult<Contract> {
        let storage = self.storage.clone();
        Box::pin(async move {
            // Check if we have it cached first
            if let Some((state, params)) = get_contract_state_cached(&id) {
                // Even if cached, request fresh state from network
                if let Some(rx) = get_contract_by_key_async(&id) {
                    match rx.await {
                        Ok(response) => {
                            if let Some(state) = response.state {
                                // Get params from cache (they don't change)
                                return Ok(Contract { state, parameters: params });
                            }
                        }
                        Err(_) => {
                            // Channel cancelled, return cached state
                        }
                    }
                }
                // Return cached state if network request failed
                return Ok(Contract { state, parameters: params });
            }

            // Contract not in cache - check if we have params in localStorage
            if let Some(params) = load_contract_params_static(&storage, &id) {
                // We have params from localStorage, use get_contract_state_async
                let rx = get_contract_state_async(&id, &params);
                match rx.await {
                    Ok(response) => {
                        if let Some(state) = response.state {
                            return Ok(Contract { state, parameters: params });
                        }
                    }
                    Err(_) => {
                        // Channel cancelled
                    }
                }
            } else {
                // No params in localStorage - try to fetch unknown contract from network
                let rx = fetch_unknown_contract_async(&id);
                match rx.await {
                    Ok(response) => {
                        if let Some(state) = response.state {
                            // Contract was found and cached by handle_get_response
                            // Get the params from the cache now
                            if let Some((_, params)) = get_contract_state_cached(&id) {
                                // Save to localStorage for future sessions
                                save_contract_key_static(&storage, &id, &params);

                                // Subscribe to updates for this newly fetched contract
                                let _ = subscribe_to_contract_async(&id);

                                return Ok(Contract { state, parameters: params });
                            }
                        }
                    }
                    Err(_) => {
                        // Channel cancelled
                    }
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
                contracts.insert(id.clone(), (new_state.clone(), params.clone(), contract_key));
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
        Ok(self.signing_key.verifying_key())
    }

    /// Returns the user's private key (signing key).
    fn get_private_key(&self) -> Result<SigningKey, Box<dyn Error>> {
        Ok(self.signing_key.clone())
    }

    /// Signs a message with the user's private key.
    fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let signature: Signature = self.signing_key.sign(message);
        Ok(signature.to_bytes().to_vec())
    }

    /// Publish a new contract to the Freenet network.
    ///
    /// This sends a PUT request to the Freenet node with the contract code,
    /// state, and parameters. It waits for the network to acknowledge the
    /// contract before returning.
    fn publish_contract(&self, contract: Contract) -> AsyncResult<PublishContractResponse> {
        let state = contract.state;
        let params = contract.parameters;
        let storage = self.storage.clone();

        Box::pin(async move {
            // Publish the contract and get a receiver for the response
            let (contract_key, response_rx) = publish_contract_async(&state, &params);

            // Wait for the PUT response
            match response_rx.await {
                Ok(response) => {
                    if response.success {
                        // Save contract key and params to localStorage
                        save_contract_key_static(&storage, &response.contract_key, &params);

                        // Subscribe to updates for this contract
                        if let Some(subscribe_rx) = subscribe_to_contract_async(&contract_key) {
                            // Wait for subscription to be confirmed (with a reasonable timeout)
                            let _ = subscribe_rx.await;
                        }

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
}

#[cfg(test)]
mod tests {
    // Tests would go here, but they require a running Freenet node
}
