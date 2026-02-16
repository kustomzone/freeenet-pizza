//! Base service interface for contract operations.
//!
//! Provides an async trait that can be implemented by different backends
//! (Freenet network, local storage for testing, etc.)

use ed25519_dalek::{SigningKey, VerifyingKey};
use futures::Stream;
use pizza_common::{FullOrderStateV1, FullOrderStateV1Delta, OrderParametersV1};
use std::error::Error;
use std::future::Future;
use std::pin::Pin;

/// Result type alias for async operations
pub type AsyncResult<T> = Pin<Box<dyn Future<Output = Result<T, Box<dyn Error>>>>>;

/// Contract data structure containing state and parameters
#[derive(Clone, PartialEq, Debug)]
pub struct Contract {
    pub state: FullOrderStateV1,
    pub parameters: OrderParametersV1,
}

/// Response from publishing a contract
#[derive(Clone, Debug)]
pub struct PublishContractResponse {
    /// The contract key (ID) assigned by the network
    pub contract_key: String,
    /// The initial state
    pub state: FullOrderStateV1,
}

/// Response from publishing a delta update
#[derive(Clone, Debug)]
pub struct PublishDeltaResponse {
    /// The new state after applying the delta
    pub state: FullOrderStateV1,
    /// Whether the update was acknowledged by the network
    pub acknowledged: bool,
}

/// Base interface for contract services.
///
/// This trait defines async operations for interacting with contracts.
/// Implementations can be backed by:
/// - Freenet network (FreenetService)
/// - Local storage (LocalStorageService) for testing
pub trait BaseInterface {
    /// Returns a list of contract IDs/keys from local cache.
    fn get_contracts(&self) -> Result<Vec<String>, Box<dyn Error>>;

    /// Returns parameters and state for a given contract ID from local cache.
    /// Returns None if contract is not in cache.
    fn get_contract_cached(&self, id: String) -> Option<Contract>;

    /// Fetch contract state from the network.
    ///
    /// Returns a future that resolves when the state is received from Freenet.
    /// The contract must already be known (via previous publish or subscription).
    fn get_contract_async(&self, id: String) -> AsyncResult<Contract>;

    /// Publish a delta update to a contract.
    ///
    /// Returns a future that resolves when the network acknowledges the update.
    fn publish_delta(&self, id: String, delta: FullOrderStateV1Delta) -> AsyncResult<PublishDeltaResponse>;

    /// Returns the public key (verifying key).
    fn get_public_key(&self) -> Result<VerifyingKey, Box<dyn Error>>;

    /// Returns the private key (signing key).
    fn get_private_key(&self) -> Result<SigningKey, Box<dyn Error>>;

    /// Signs a message and returns the signature.
    fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>>;

    /// Publish a new contract to the network.
    ///
    /// Returns a future that resolves when the contract is published
    /// and acknowledged by the network.
    fn publish_contract(&self, contract: Contract) -> AsyncResult<PublishContractResponse>;

    /// Subscribe to contract list changes.
    ///
    /// Returns a stream that emits the list of contract keys whenever it changes.
    fn subscribe_contracts(&self) -> Pin<Box<dyn Stream<Item = Vec<String>>>>;

    /// Subscribe to contract state changes.
    ///
    /// Returns a stream that emits the contract state whenever it changes.
    fn subscribe_contract_state(&self, id: String) -> Pin<Box<dyn Stream<Item = Contract>>>;
}

/// Wrapper type for BaseInterface implementations.
///
/// Allows storing the service in Dioxus context.
#[derive(Clone)]
pub struct BaseService(pub std::rc::Rc<dyn BaseInterface>);

impl std::ops::Deref for BaseService {
    type Target = dyn BaseInterface;
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}
