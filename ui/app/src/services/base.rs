use std::error::Error;
use ed25519_dalek::{SigningKey, VerifyingKey};
use pizza_common::{FullOrderStateV1, FullOrderStateV1Delta, OrderParametersV1};
use futures::Stream;
use std::pin::Pin;

#[derive(Clone, PartialEq)]
pub struct Contract {
    pub state: FullOrderStateV1,
    pub parameters: OrderParametersV1,
}

pub trait BaseInterface {
    /// Returns a list of contract IDs or names
    fn get_contracts(&self) -> Result<Vec<String>, Box<dyn Error>>;

    /// Returns parameters and state for a given contract ID
    fn get_contract_parameters_and_state(
        &self,
        id: String,
    ) -> Result<Contract, Box<dyn Error>>;

    /// Publish delta
    fn publish_delta(&self, id: String, delta: FullOrderStateV1Delta
    ) -> Result<FullOrderStateV1, Box<dyn Error>>;

    /// Returns the public key as bytes or encoded string
    fn get_public_key(&self) -> Result<VerifyingKey, Box<dyn Error>>;

    /// Returns the private key as bytes or encoded string
    fn get_private_key(&self) -> Result<SigningKey, Box<dyn Error>>;

    /// Signs a message and returns the signature
    fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>>;

    /// Publish a new contract
    fn publish_contract(
        &self,
        contract: Contract,
    ) -> Result<FullOrderStateV1, Box<dyn Error>>;

    /// Subscribe to contract list changes
    fn subscribe_contracts(&self) -> Pin<Box<dyn Stream<Item = Vec<String>>>>;

    /// Subscribe to contract state changes
    fn subscribe_contract_state(&self, id: String) -> Pin<Box<dyn Stream<Item = Contract>>>;
}

#[derive(Clone)]
pub struct BaseService(pub std::rc::Rc<dyn BaseInterface>);

impl std::ops::Deref for BaseService {
    type Target = dyn BaseInterface;
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}