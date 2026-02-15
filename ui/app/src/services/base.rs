use std::error::Error;
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer};
use pizza_common::{FullOrderStateV1, FullOrderStateV1Delta, OrderParametersV1, ComposableState};
use web_sys::Storage;
use serde_json;
use wasm_bindgen::JsValue;

fn js_to_err(js: JsValue) -> Box<dyn Error> {
    format!("{:?}", js).into()
}

pub trait BaseInterface {
    /// Returns a list of contract IDs or names
    fn get_contracts(&self) -> Result<Vec<String>, Box<dyn Error>>;

    /// Returns parameters and state for a given contract ID
    fn get_contract_parameters_and_state(
        &self,
        id: String,
    ) -> Result<(FullOrderStateV1, OrderParametersV1), Box<dyn Error>>;

    /// Publish delta
    fn publish_delta(&self, id: String, delta: FullOrderStateV1Delta
    ) -> Result<FullOrderStateV1, Box<dyn Error>>;

    /// Returns the public key as bytes or encoded string
    fn get_public_key(&self) -> Result<VerifyingKey, Box<dyn Error>>;

    /// Returns the private key as bytes or encoded string
    fn get_private_key(&self) -> Result<SigningKey, Box<dyn Error>>;

    /// Signs a message and returns the signature
    fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>>;
}

pub struct LocalStorageService {
    storage: Storage,
}

const CONTRACTS_KEY: &str = "pizza_contracts";
const PRIVATE_KEY_KEY: &str = "pizza_private_key";

impl LocalStorageService {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let window = web_sys::window().ok_or("no window")?;
        let storage = window.local_storage().map_err(js_to_err)?.ok_or("no local storage")?;
        Ok(Self { storage })
    }

    fn get_state_key(id: &str) -> String {
        format!("pizza_contract_state_{}", id)
    }

    fn get_params_key(id: &str) -> String {
        format!("pizza_contract_params_{}", id)
    }
}

impl BaseInterface for LocalStorageService {
    fn get_contracts(&self) -> Result<Vec<String>, Box<dyn Error>> {
        match self.storage.get_item(CONTRACTS_KEY).map_err(js_to_err)? {
            Some(s) => Ok(serde_json::from_str(&s)?),
            None => Ok(vec![]),
        }
    }

    fn get_contract_parameters_and_state(
        &self,
        id: String,
    ) -> Result<(FullOrderStateV1, OrderParametersV1), Box<dyn Error>> {
        let state_json = self.storage.get_item(&Self::get_state_key(&id)).map_err(js_to_err)?
            .ok_or(format!("state not found for {}", id))?;
        let params_json = self.storage.get_item(&Self::get_params_key(&id)).map_err(js_to_err)?
            .ok_or(format!("params not found for {}", id))?;

        let state: FullOrderStateV1 = serde_json::from_str(&state_json)?;
        let params: OrderParametersV1 = serde_json::from_str(&params_json)?;

        Ok((state, params))
    }

    fn publish_delta(&self, id: String, delta: FullOrderStateV1Delta
    ) -> Result<FullOrderStateV1, Box<dyn Error>> {
        let (state, params) = self.get_contract_parameters_and_state(id.clone())?;
        let mut new_state = state.clone();
        new_state.apply_delta(&state, &params, &Some(delta)).map_err(|e| e.to_string())?;

        let state_json = serde_json::to_string(&new_state)?;
        self.storage.set_item(&Self::get_state_key(&id), &state_json).map_err(js_to_err)?;

        Ok(new_state)
    }

    fn get_public_key(&self) -> Result<VerifyingKey, Box<dyn Error>> {
        let signing_key = self.get_private_key()?;
        Ok(signing_key.verifying_key())
    }

    fn get_private_key(&self) -> Result<SigningKey, Box<dyn Error>> {
        match self.storage.get_item(PRIVATE_KEY_KEY).map_err(js_to_err)? {
            Some(s) => {
                let bytes = hex::decode(s)?;
                let bytes: [u8; 32] = bytes.try_into().map_err(|_| "invalid private key length")?;
                Ok(SigningKey::from_bytes(&bytes))
            }
            None => {
                let mut rng = rand::thread_rng();
                let signing_key = SigningKey::generate(&mut rng);
                let hex_key = hex::encode(signing_key.to_bytes());
                self.storage.set_item(PRIVATE_KEY_KEY, &hex_key).map_err(js_to_err)?;
                Ok(signing_key)
            }
        }
    }

    fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let signing_key = self.get_private_key()?;
        let signature: Signature = signing_key.sign(message);
        Ok(signature.to_bytes().to_vec())
    }
}