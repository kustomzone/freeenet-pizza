use std::error::Error;
use std::pin::Pin;
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer};
use pizza_common::{FullOrderStateV1, FullOrderStateV1Delta, OrderParametersV1};
use futures::Stream;
use freenet_stdlib::client_api::{ClientRequest, ContractRequest, HostResponse, ContractResponse};
use freenet_stdlib::prelude::ContractKey;

use super::base::{BaseInterface, Contract};
use crate::api::node_api::{with_current_ws, send_request};
use crate::services::local_storage::LocalStorageService;

pub struct FreenetService {
    local_storage: LocalStorageService,
}

impl FreenetService {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            local_storage: LocalStorageService::new()?,
        })
    }
}

impl BaseInterface for FreenetService {
    fn get_contracts(&self) -> Result<Vec<String>, Box<dyn Error>> {
        // For now, we might still want to track which contracts we are interested in locally,
        // or we could try to list them from the node if there's a way.
        // Assuming we keep a local list of "joined" or "created" contracts for now.
        self.local_storage.get_contracts()
    }

    fn get_contract_parameters_and_state(
        &self,
        id: String,
    ) -> Result<Contract, Box<dyn Error>> {
        // TODO: Implement fetching from Freenet node.
        // This will likely need to be async or handled via subscriptions.
        // For now, falling back to local storage or returning error if not present.
        self.local_storage.get_contract_parameters_and_state(id)
    }

    fn publish_delta(&self, id: String, delta: FullOrderStateV1Delta
    ) -> Result<FullOrderStateV1, Box<dyn Error>> {
        // TODO: Send UpdateRequest to Freenet node.
        let _contract = self.get_contract_parameters_and_state(id.clone())?;
        // We still need to compute the new state locally to return it, 
        // or wait for the node to confirm.
        self.local_storage.publish_delta(id, delta)
    }

    fn get_public_key(&self) -> Result<VerifyingKey, Box<dyn Error>> {
        self.local_storage.get_public_key()
    }

    fn get_private_key(&self) -> Result<SigningKey, Box<dyn Error>> {
        self.local_storage.get_private_key()
    }

    fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        self.local_storage.sign_message(message)
    }

    fn publish_contract(
        &self,
        contract: Contract,
    ) -> Result<FullOrderStateV1, Box<dyn Error>> {
        // TODO: Send PutRequest to Freenet node.
        self.local_storage.publish_contract(contract)
    }

    fn subscribe_contracts(&self) -> Pin<Box<dyn Stream<Item = Vec<String>>>> {
        self.local_storage.subscribe_contracts()
    }

    fn subscribe_contract_state(&self, id: String) -> Pin<Box<dyn Stream<Item = Contract>>> {
        // TODO: Subscribe to UpdateNotifications from Freenet node.
        self.local_storage.subscribe_contract_state(id)
    }
}
