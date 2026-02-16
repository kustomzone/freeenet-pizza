//! Local Storage Service - Test/offline implementation of BaseInterface.
//!
//! This service uses browser localStorage for persistence and is useful for
//! testing and offline development. It doesn't require a Freenet node.

use std::collections::HashMap;
use std::error::Error;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use futures::Stream;
use pizza_common::{ComposableState, FullOrderStateV1, FullOrderStateV1Delta, OrderParametersV1};
use serde_json;
use wasm_bindgen::JsValue;
use web_sys::Storage;

use super::base::{
    AsyncResult, BaseInterface, Contract, PublishContractResponse, PublishDeltaResponse,
};

fn js_to_err(js: JsValue) -> Box<dyn Error> {
    format!("{:?}", js).into()
}

/// LocalStorageService provides a localStorage-based implementation of BaseInterface.
///
/// This is useful for:
/// - Local development without a Freenet node
/// - Testing UI components
/// - Offline mode
#[derive(Clone)]
pub struct LocalStorageService {
    storage: Storage,
    contract_subscribers:
        Arc<Mutex<Vec<futures::channel::mpsc::UnboundedSender<Vec<String>>>>>,
    state_subscribers:
        Arc<Mutex<HashMap<String, Vec<futures::channel::mpsc::UnboundedSender<Contract>>>>>,
}

const CONTRACTS_KEY: &str = "pizza_contracts";
const PRIVATE_KEY_KEY: &str = "pizza_private_key";

impl LocalStorageService {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let window = web_sys::window().ok_or("no window")?;
        let storage = window
            .local_storage()
            .map_err(js_to_err)?
            .ok_or("no local storage")?;
        Ok(Self {
            storage,
            contract_subscribers: Arc::new(Mutex::new(Vec::new())),
            state_subscribers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn get_state_key(id: &str) -> String {
        format!("pizza_contract_state_{}", id)
    }

    fn get_params_key(id: &str) -> String {
        format!("pizza_contract_params_{}", id)
    }

    fn notify_contract_subscribers(&self, contracts: Vec<String>) {
        let mut subs = self.contract_subscribers.lock().unwrap();
        subs.retain(|sub| sub.unbounded_send(contracts.clone()).is_ok());
    }

    fn notify_state_subscribers(
        &self,
        id: String,
        state: FullOrderStateV1,
        params: OrderParametersV1,
    ) {
        let mut all_subs = self.state_subscribers.lock().unwrap();
        if let Some(subs) = all_subs.get_mut(&id) {
            subs.retain(|sub| {
                sub.unbounded_send(Contract {
                    state: state.clone(),
                    parameters: params.clone(),
                })
                .is_ok()
            });
        }
    }
}

impl BaseInterface for LocalStorageService {
    fn get_contracts(&self) -> Result<Vec<String>, Box<dyn Error>> {
        match self.storage.get_item(CONTRACTS_KEY).map_err(js_to_err)? {
            Some(s) => Ok(serde_json::from_str(&s)?),
            None => Ok(vec![]),
        }
    }

    fn get_contract_cached(&self, id: String) -> Option<Contract> {
        let state_json = self
            .storage
            .get_item(&Self::get_state_key(&id))
            .ok()??;
        let params_json = self
            .storage
            .get_item(&Self::get_params_key(&id))
            .ok()??;

        let state: FullOrderStateV1 = serde_json::from_str(&state_json).ok()?;
        let params: OrderParametersV1 = serde_json::from_str(&params_json).ok()?;

        Some(Contract {
            state,
            parameters: params,
        })
    }

    fn get_contract_async(&self, id: String) -> AsyncResult<Contract> {
        let storage = self.storage.clone();
        Box::pin(async move {
            let state_json = storage
                .get_item(&Self::get_state_key(&id))
                .map_err(js_to_err)?
                .ok_or(format!("state not found for {}", id))?;
            let params_json = storage
                .get_item(&Self::get_params_key(&id))
                .map_err(js_to_err)?
                .ok_or(format!("params not found for {}", id))?;

            let state: FullOrderStateV1 = serde_json::from_str(&state_json)?;
            let params: OrderParametersV1 = serde_json::from_str(&params_json)?;

            Ok(Contract {
                state,
                parameters: params,
            })
        })
    }

    fn publish_delta(
        &self,
        id: String,
        delta: FullOrderStateV1Delta,
    ) -> AsyncResult<PublishDeltaResponse> {
        // Clone self for the async block
        let storage = self.storage.clone();
        let state_subscribers = self.state_subscribers.clone();

        Box::pin(async move {
            // Get current contract
            let state_json = storage
                .get_item(&Self::get_state_key(&id))
                .map_err(js_to_err)?
                .ok_or(format!("state not found for {}", id))?;
            let params_json = storage
                .get_item(&Self::get_params_key(&id))
                .map_err(js_to_err)?
                .ok_or(format!("params not found for {}", id))?;

            let current_state: FullOrderStateV1 = serde_json::from_str(&state_json)?;
            let params: OrderParametersV1 = serde_json::from_str(&params_json)?;

            // Apply delta
            let mut new_state = current_state.clone();
            new_state
                .apply_delta(&current_state, &params, &Some(delta))
                .map_err(|e| e.to_string())?;

            // Save to storage
            let new_state_json = serde_json::to_string(&new_state)?;
            storage
                .set_item(&Self::get_state_key(&id), &new_state_json)
                .map_err(js_to_err)?;

            // Notify subscribers
            {
                let mut all_subs = state_subscribers.lock().unwrap();
                if let Some(subs) = all_subs.get_mut(&id) {
                    subs.retain(|sub| {
                        sub.unbounded_send(Contract {
                            state: new_state.clone(),
                            parameters: params.clone(),
                        })
                        .is_ok()
                    });
                }
            }

            Ok(PublishDeltaResponse {
                state: new_state,
                acknowledged: true, // Local storage always succeeds
            })
        })
    }

    fn get_public_key(&self) -> Result<VerifyingKey, Box<dyn Error>> {
        let signing_key = self.get_private_key()?;
        Ok(signing_key.verifying_key())
    }

    fn get_private_key(&self) -> Result<SigningKey, Box<dyn Error>> {
        match self.storage.get_item(PRIVATE_KEY_KEY).map_err(js_to_err)? {
            Some(s) => {
                let bytes = hex::decode(s)?;
                let bytes: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| "invalid private key length")?;
                Ok(SigningKey::from_bytes(&bytes))
            }
            None => {
                let mut rng = rand::thread_rng();
                let signing_key = SigningKey::generate(&mut rng);
                let hex_key = hex::encode(signing_key.to_bytes());
                self.storage
                    .set_item(PRIVATE_KEY_KEY, &hex_key)
                    .map_err(js_to_err)?;
                Ok(signing_key)
            }
        }
    }

    fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let signing_key = self.get_private_key()?;
        let signature: Signature = signing_key.sign(message);
        Ok(signature.to_bytes().to_vec())
    }

    fn publish_contract(&self, contract: Contract) -> AsyncResult<PublishContractResponse> {
        // Clone self for the async block
        let storage = self.storage.clone();
        let contract_subscribers = self.contract_subscribers.clone();
        let state_subscribers = self.state_subscribers.clone();

        Box::pin(async move {
            let id = format!("{}", rand::random::<u32>());
            let state_json = serde_json::to_string(&contract.state)?;
            let params_json = serde_json::to_string(&contract.parameters)?;

            storage
                .set_item(&Self::get_state_key(&id), &state_json)
                .map_err(js_to_err)?;
            storage
                .set_item(&Self::get_params_key(&id), &params_json)
                .map_err(js_to_err)?;

            // Update contracts list
            let contracts: Vec<String> = match storage.get_item(CONTRACTS_KEY).map_err(js_to_err)? {
                Some(s) => serde_json::from_str(&s)?,
                None => vec![],
            };
            let mut contracts = contracts;
            contracts.push(id.clone());
            let contracts_json = serde_json::to_string(&contracts)?;
            storage
                .set_item(CONTRACTS_KEY, &contracts_json)
                .map_err(js_to_err)?;

            // Notify contract list subscribers
            {
                let mut subs = contract_subscribers.lock().unwrap();
                subs.retain(|sub| sub.unbounded_send(contracts.clone()).is_ok());
            }

            // Notify state subscribers
            {
                let mut all_subs = state_subscribers.lock().unwrap();
                if let Some(subs) = all_subs.get_mut(&id) {
                    subs.retain(|sub| {
                        sub.unbounded_send(Contract {
                            state: contract.state.clone(),
                            parameters: contract.parameters.clone(),
                        })
                        .is_ok()
                    });
                }
            }

            Ok(PublishContractResponse {
                contract_key: id,
                state: contract.state,
            })
        })
    }

    fn subscribe_contracts(&self) -> Pin<Box<dyn Stream<Item = Vec<String>>>> {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        let mut subs = self.contract_subscribers.lock().unwrap();
        subs.push(tx);
        Box::pin(rx)
    }

    fn subscribe_contract_state(&self, id: String) -> Pin<Box<dyn Stream<Item = Contract>>> {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        let mut all_subs = self.state_subscribers.lock().unwrap();
        all_subs.entry(id).or_insert_with(Vec::new).push(tx);
        Box::pin(rx)
    }
}
