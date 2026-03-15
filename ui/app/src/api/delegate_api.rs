//! Pizza Delegate API - Communication with the pizza-delegate for private state storage.
//!
//! This module provides async communication with the pizza-delegate for storing
//! contract keys and signing key in Freenet secret storage.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use ciborium::ser::into_writer;
use freenet_stdlib::client_api::ClientRequest::DelegateOp;
use freenet_stdlib::client_api::DelegateRequest;
use freenet_stdlib::prelude::tracing::{error, info};
use freenet_stdlib::prelude::{
    ApplicationMessage, Delegate, DelegateCode, DelegateContainer, DelegateWasmAPIVersion,
    InboundDelegateMsg, Parameters,
};
use futures::channel::oneshot;
use futures::future::{select, Either};
use pizza_common::order_delegate::{PizzaDelegateRequest, PizzaDelegateResponse, RequestId};

/// Delegate WASM bytes - embedded at compile time
pub const DELEGATE_WASM: &[u8] =
    include_bytes!("../../../../delegates/pizza-delegate/build/pizza_delegate.wasm");

/// Atomic counter for generating unique request IDs
static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a unique request ID for signing requests
pub fn generate_request_id() -> RequestId {
    REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

// Request tracking keys
const CONTRACT_KEYS_KEY: &[u8] = b"__contract_keys__";
const SIGNING_KEY_KEY: &[u8] = b"__signing_key__";
const PUBLIC_KEY_KEY: &[u8] = b"__public_key__";
const SIGN_PREFIX: &[u8] = b"__sign:";

/// Registry for pending delegate requests.
/// Maps request keys to oneshot senders that will receive the response.
static PENDING_REQUESTS: std::sync::LazyLock<
    Mutex<HashMap<Vec<u8>, oneshot::Sender<PizzaDelegateResponse>>>,
> = std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Complete a pending contract keys request.
pub fn complete_pending_contract_keys_request(response: PizzaDelegateResponse) -> bool {
    complete_pending_request_bytes(CONTRACT_KEYS_KEY, response)
}

/// Complete a pending signing key store request.
pub fn complete_pending_signing_key_request(response: PizzaDelegateResponse) -> bool {
    complete_pending_request_bytes(SIGNING_KEY_KEY, response)
}

/// Complete a pending public key request.
pub fn complete_pending_public_key_request(response: PizzaDelegateResponse) -> bool {
    complete_pending_request_bytes(PUBLIC_KEY_KEY, response)
}

/// Complete a pending sign request.
pub fn complete_pending_sign_request(
    request_id: RequestId,
    response: PizzaDelegateResponse,
) -> bool {
    let mut key_bytes = SIGN_PREFIX.to_vec();
    key_bytes.extend_from_slice(&request_id.to_le_bytes());
    complete_pending_request_bytes(&key_bytes, response)
}

/// Internal function to complete a pending request by key bytes.
fn complete_pending_request_bytes(key_bytes: &[u8], response: PizzaDelegateResponse) -> bool {
    if let Ok(mut pending) = PENDING_REQUESTS.lock() {
        if let Some(sender) = pending.remove(key_bytes) {
            if sender.send(response).is_ok() {
                info!(
                    "Completed pending request for key: {:?}",
                    String::from_utf8_lossy(key_bytes)
                );
                return true;
            }
        }
    }
    false
}

/// Create the pizza delegate container for registration.
pub fn create_pizza_delegate_container() -> DelegateContainer {
    let delegate_code = DelegateCode::from(DELEGATE_WASM.to_vec());
    let params = Parameters::from(Vec::<u8>::new());
    let delegate = Delegate::from((&delegate_code, &params));
    DelegateContainer::Wasm(DelegateWasmAPIVersion::V1(delegate))
}

/// Get the delegate key for the pizza delegate.
pub fn get_delegate_key() -> freenet_stdlib::prelude::DelegateKey {
    let delegate_code = DelegateCode::from(DELEGATE_WASM.to_vec());
    let params = Parameters::from(Vec::<u8>::new());
    let delegate = Delegate::from((&delegate_code, &params));
    delegate.key().clone()
}

/// Get the request tracking key for a request.
fn get_request_key(request: &PizzaDelegateRequest) -> Vec<u8> {
    match request {
        PizzaDelegateRequest::StoreContractKeys { .. } => CONTRACT_KEYS_KEY.to_vec(),
        PizzaDelegateRequest::GetContractKeys => CONTRACT_KEYS_KEY.to_vec(),
        PizzaDelegateRequest::StoreSigningKey { .. } => SIGNING_KEY_KEY.to_vec(),
        PizzaDelegateRequest::GetSigningKey => SIGNING_KEY_KEY.to_vec(),
        PizzaDelegateRequest::GetPublicKey => PUBLIC_KEY_KEY.to_vec(),
        PizzaDelegateRequest::Sign { request_id, .. } => {
            let mut key = SIGN_PREFIX.to_vec();
            key.extend_from_slice(&request_id.to_le_bytes());
            key
        }
    }
}

/// Send a request to the delegate and wait for the response.
pub async fn send_delegate_request(
    request: PizzaDelegateRequest,
) -> Result<PizzaDelegateResponse, String> {
    info!("Sending delegate request: {:?}", request);

    // Get the key bytes for tracking this request
    let key_bytes = get_request_key(&request);

    // Create a oneshot channel to receive the response
    let (sender, receiver) = oneshot::channel();

    // Register the pending request
    {
        let mut pending = PENDING_REQUESTS
            .lock()
            .map_err(|e| format!("Failed to lock pending requests: {}", e))?;
        pending.insert(key_bytes.clone(), sender);
    }

    // Serialize the request
    let mut payload = Vec::new();
    into_writer(&request, &mut payload)
        .map_err(|e| format!("Failed to serialize request: {}", e))?;

    info!("Serialized request payload size: {} bytes", payload.len());

    let delegate_key = get_delegate_key();
    let app_msg = ApplicationMessage::new(payload);

    // Prepare the delegate request
    let delegate_request = DelegateOp(DelegateRequest::ApplicationMessages {
        key: delegate_key,
        params: Parameters::from(Vec::<u8>::new()),
        inbound: vec![InboundDelegateMsg::ApplicationMessage(app_msg)],
    });

    // Send via WebSocket
    send_delegate_request_via_ws(&delegate_request);

    info!("Request sent, waiting for response...");

    // Wait for the response with a timeout (10 seconds)
    let timeout = Box::pin(sleep_ms(10_000));

    match select(receiver, timeout).await {
        Either::Left((response, _)) => match response {
            Ok(resp) => {
                info!("Received delegate response: {:?}", resp);
                Ok(resp)
            }
            Err(_) => Err("Response channel was cancelled".to_string()),
        },
        Either::Right((_, _)) => {
            // Timeout occurred - remove the pending request
            if let Ok(mut pending) = PENDING_REQUESTS.lock() {
                pending.remove(&key_bytes);
            }
            Err("Timeout waiting for delegate response".to_string())
        }
    }
}

/// Fire a request to the delegate without waiting for response.
/// Used during initialization to avoid deadlocks in the message loop.
pub fn fire_delegate_request(request: PizzaDelegateRequest) {
    info!("Firing delegate request (fire and forget): {:?}", request);

    // Serialize the request
    let mut payload = Vec::new();
    if let Err(e) = into_writer(&request, &mut payload) {
        error!("Failed to serialize delegate request: {}", e);
        return;
    }

    let delegate_key = get_delegate_key();
    let app_msg = ApplicationMessage::new(payload);

    let delegate_request = DelegateOp(DelegateRequest::ApplicationMessages {
        key: delegate_key,
        params: Parameters::from(Vec::<u8>::new()),
        inbound: vec![InboundDelegateMsg::ApplicationMessage(app_msg)],
    });

    send_delegate_request_via_ws(&delegate_request);
}

/// Register the delegate with the Freenet node.
pub fn register_delegate() {
    info!("Registering pizza delegate");
    let delegate = create_pizza_delegate_container();

    let register_request = DelegateOp(DelegateRequest::RegisterDelegate {
        delegate,
        cipher: DelegateRequest::DEFAULT_CIPHER,
        nonce: DelegateRequest::DEFAULT_NONCE,
    });

    send_delegate_request_via_ws(&register_request);
}

/// Send delegate request via WebSocket using the existing node API
fn send_delegate_request_via_ws(request: &freenet_stdlib::client_api::ClientRequest) {
    use freenet_stdlib::prelude::bincode;

    match bincode::serialize(request) {
        Ok(bytes) => {
            // Use the existing send mechanism from node_api
            super::node_api::with_current_ws(|ws| {
                if let Err(e) = ws.send_with_u8_array(&bytes) {
                    error!("Failed to send delegate request: {:?}", e);
                }
            });

            // If WebSocket not ready, queue the request
            if !super::node_api::is_connected() {
                super::node_api::PENDING_SEND_QUEUE.with(|queue| {
                    queue.borrow_mut().push(bytes);
                });
            }
        }
        Err(e) => {
            error!("Failed to serialize delegate request: {}", e);
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

// ============================================================================
// High-level API functions
// ============================================================================

/// Store contract keys in the delegate.
pub async fn store_contract_keys(keys: Vec<String>) -> Result<(), String> {
    let request = PizzaDelegateRequest::StoreContractKeys { keys };

    match send_delegate_request(request).await? {
        PizzaDelegateResponse::StoreContractKeysResponse { result } => result,
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Load contract keys from the delegate.
pub async fn load_contract_keys() -> Result<Vec<String>, String> {
    let request = PizzaDelegateRequest::GetContractKeys;

    match send_delegate_request(request).await? {
        PizzaDelegateResponse::GetContractKeysResponse { keys } => Ok(keys),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Store the signing key in the delegate.
pub async fn store_signing_key(signing_key_bytes: [u8; 32]) -> Result<(), String> {
    let request = PizzaDelegateRequest::StoreSigningKey { signing_key_bytes };

    match send_delegate_request(request).await? {
        PizzaDelegateResponse::StoreSigningKeyResponse { result } => result,
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Get the stored signing key from the delegate.
pub async fn get_signing_key() -> Result<Option<[u8; 32]>, String> {
    let request = PizzaDelegateRequest::GetSigningKey;

    match send_delegate_request(request).await? {
        PizzaDelegateResponse::GetSigningKeyResponse { signing_key } => Ok(signing_key),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Get the public key from the stored signing key.
pub async fn get_public_key() -> Result<Option<[u8; 32]>, String> {
    let request = PizzaDelegateRequest::GetPublicKey;

    match send_delegate_request(request).await? {
        PizzaDelegateResponse::GetPublicKeyResponse { public_key } => Ok(public_key),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Sign data with the stored signing key.
pub async fn sign_data(data: Vec<u8>) -> Result<Vec<u8>, String> {
    let request_id = generate_request_id();
    let request = PizzaDelegateRequest::Sign { request_id, data };

    match send_delegate_request(request).await? {
        PizzaDelegateResponse::SignResponse { signature, .. } => signature,
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Fire a request to load contract keys without waiting (for initialization).
pub fn fire_load_contract_keys_request() {
    fire_delegate_request(PizzaDelegateRequest::GetContractKeys);
}

/// Fire a request to get public key without waiting (for initialization).
pub fn fire_get_public_key_request() {
    fire_delegate_request(PizzaDelegateRequest::GetPublicKey);
}
