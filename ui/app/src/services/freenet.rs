//! Freenet Service - High-level service for managing pizza order contracts on Freenet.
//!
//! This service implements the BaseInterface trait using the Freenet node API
//! for publishing, subscribing, and updating contracts.

use std::error::Error;
use std::pin::Pin;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use freenet_stdlib::prelude::ContractKey;
use futures::Stream;
use futures::StreamExt;
use pizza_common::{ComposableState, FullOrderStateV1, FullOrderStateV1Delta, OrderParametersV1};

use super::base::{BaseInterface, Contract};
use crate::api::node_api::{
    get_contract_keys, get_contract_state, publish_contract as api_publish_contract,
    send_contract_update, subscribe_to_contract, subscribe_to_contract_list,
    subscribe_to_contract_updates, CONTRACTS,
};

/// Private key storage key in browser storage (for key persistence across sessions)
const PRIVATE_KEY_KEY: &str = "pizza_private_key";

/// FreenetService implements the BaseInterface trait using the Freenet node API.
///
/// Unlike LocalStorageService, this service:
/// - Publishes contracts to the Freenet network via PUT requests
/// - Subscribes to contract updates via WebSocket notifications
/// - Sends state updates to the network
/// - Uses browser storage only for private key persistence (not contract state)
#[derive(Clone)]
pub struct FreenetService {
    /// The user's signing key (persisted in browser storage)
    signing_key: SigningKey,
}

impl FreenetService {
    /// Create a new FreenetService.
    ///
    /// This will load the signing key from browser storage or generate a new one.
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let signing_key = Self::load_or_create_signing_key()?;
        Ok(Self { signing_key })
    }

    /// Load the signing key from browser storage or create a new one.
    fn load_or_create_signing_key() -> Result<SigningKey, Box<dyn Error>> {
        let window = web_sys::window().ok_or("no window")?;
        let storage = window
            .local_storage()
            .map_err(|e| format!("{:?}", e))?
            .ok_or("no local storage")?;

        match storage.get_item(PRIVATE_KEY_KEY).map_err(|e| format!("{:?}", e))? {
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
}

impl BaseInterface for FreenetService {
    /// Returns a list of contract IDs (keys) from the Freenet node state.
    fn get_contracts(&self) -> Result<Vec<String>, Box<dyn Error>> {
        Ok(get_contract_keys())
    }

    /// Returns parameters and state for a given contract ID from the Freenet node state.
    fn get_contract_parameters_and_state(&self, id: String) -> Result<Contract, Box<dyn Error>> {
        match get_contract_state(&id) {
            Some((state, parameters)) => Ok(Contract { state, parameters }),
            None => Err(format!("Contract not found: {}", id).into()),
        }
    }

    /// Publish a delta update to a contract on the Freenet network.
    ///
    /// This computes the new state locally, then sends an update to the network.
    fn publish_delta(
        &self,
        id: String,
        delta: FullOrderStateV1Delta,
    ) -> Result<FullOrderStateV1, Box<dyn Error>> {
        use dioxus::prelude::{Readable, ReadableExt, Writable};

        // Get current state and contract key
        let (current_state, params, contract_key): (FullOrderStateV1, OrderParametersV1, ContractKey) = {
            let contracts = CONTRACTS.read();
            contracts
                .get(&id)
                .cloned()
                .ok_or_else(|| format!("Contract not found: {}", id))?
        };

        // Compute new state by applying delta
        let mut new_state: FullOrderStateV1 = current_state.clone();
        new_state
            .apply_delta(&current_state, &params, &Some(delta))
            .map_err(|e| e.to_string())?;

        // Update local state
        {
            let mut contracts = CONTRACTS.write();
            contracts.insert(
                id.clone(),
                (new_state.clone(), params.clone(), contract_key),
            );
        }

        // Send update to the network
        send_contract_update(&id, &new_state);

        Ok(new_state)
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
    /// state, and parameters. The node will then propagate the contract to
    /// the network.
    fn publish_contract(&self, contract: Contract) -> Result<FullOrderStateV1, Box<dyn Error>> {
        let contract_key = api_publish_contract(&contract.state, &contract.parameters);

        // Subscribe to updates for this contract
        subscribe_to_contract(&contract_key);

        Ok(contract.state)
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
