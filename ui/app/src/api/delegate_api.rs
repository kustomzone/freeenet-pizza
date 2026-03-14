//! Pizza Delegate API - Communication with the pizza-delegate for private state storage.
//!
//! This module provides async communication with the pizza-delegate for storing
//! contract keys, signing keys, and performing signing operations without exposing
//! private keys to the browser.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use ciborium::{de::from_reader, ser::into_writer};
use freenet_stdlib::client_api::ClientRequest::DelegateOp;
use freenet_stdlib::client_api::DelegateRequest;
use freenet_stdlib::prelude::{
    ApplicationMessage, ContractInstanceId, Delegate, DelegateCode, DelegateContainer,
    DelegateWasmAPIVersion, InboundDelegateMsg, Parameters,
};
use freenet_stdlib::prelude::tracing::{error, info};
use futures::channel::oneshot;
use futures::future::{select, Either};
use pizza_common::order_delegate::{
    OrderDelegateKey, OrderDelegateRequestMsg, OrderDelegateResponseMsg, RequestId, RoomKey,
};

// Dummy contract instance ID used when sending messages to delegate
// The delegate will receive the actual origin from the attested parameter
const DUMMY_CONTRACT_ID: [u8; 32] = [0u8; 32];

/// Delegate WASM bytes - embedded at compile time
pub const DELEGATE_WASM: &[u8] =
    include_bytes!("../../../../delegates/pizza-delegate/build/pizza_delegate.wasm");

/// Storage key for contract keys list
pub const CONTRACT_KEYS_STORAGE_KEY: &[u8] = b"contract_keys";

/// Storage key for signing key
pub const SIGNING_KEY_STORAGE_KEY: &[u8] = b"signing_key";

// Prefixes for different pending request types
const SIGNING_KEY_PREFIX: &[u8] = b"__signing_key:";
const PUBLIC_KEY_PREFIX: &[u8] = b"__public_key:";
const SIGN_PREFIX: &[u8] = b"__sign:";

/// Atomic counter for generating unique request IDs
static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a unique request ID for signing requests
pub fn generate_request_id() -> RequestId {
    REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Registry for pending delegate requests.
/// Maps request keys to oneshot senders that will receive the response.
static PENDING_REQUESTS: std::sync::LazyLock<
    Mutex<HashMap<Vec<u8>, oneshot::Sender<OrderDelegateResponseMsg>>>,
> = std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Complete a pending delegate request with the given response.
/// Called by the response handler when a delegate response is received.
pub fn complete_pending_request(key: &OrderDelegateKey, response: OrderDelegateResponseMsg) -> bool {
    let key_bytes = key.as_bytes().to_vec();
    complete_pending_request_bytes(&key_bytes, response)
}

/// Complete a pending signing key store request.
pub fn complete_pending_signing_key_request(
    room_key: &RoomKey,
    response: OrderDelegateResponseMsg,
) -> bool {
    let mut key_bytes = SIGNING_KEY_PREFIX.to_vec();
    key_bytes.extend_from_slice(room_key);
    complete_pending_request_bytes(&key_bytes, response)
}

/// Complete a pending public key request.
pub fn complete_pending_public_key_request(
    room_key: &RoomKey,
    response: OrderDelegateResponseMsg,
) -> bool {
    let mut key_bytes = PUBLIC_KEY_PREFIX.to_vec();
    key_bytes.extend_from_slice(room_key);
    complete_pending_request_bytes(&key_bytes, response)
}

/// Complete a pending signing request using room_key and request_id for correlation.
pub fn complete_pending_sign_request(
    room_key: &RoomKey,
    request_id: RequestId,
    response: OrderDelegateResponseMsg,
) -> bool {
    let mut key_bytes = SIGN_PREFIX.to_vec();
    key_bytes.extend_from_slice(room_key);
    key_bytes.extend_from_slice(&request_id.to_le_bytes());
    complete_pending_request_bytes(&key_bytes, response)
}

/// Internal function to complete a pending request by key bytes.
fn complete_pending_request_bytes(key_bytes: &[u8], response: OrderDelegateResponseMsg) -> bool {
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

/// Extract the key from a request message for tracking purposes.
fn get_request_key(request: &OrderDelegateRequestMsg) -> Vec<u8> {
    match request {
        // Key-value storage operations
        OrderDelegateRequestMsg::StoreRequest { key, .. } => key.as_bytes().to_vec(),
        OrderDelegateRequestMsg::GetRequest { key } => key.as_bytes().to_vec(),
        OrderDelegateRequestMsg::DeleteRequest { key } => key.as_bytes().to_vec(),
        OrderDelegateRequestMsg::ListRequest => b"__list_request__".to_vec(),

        // Signing key management
        OrderDelegateRequestMsg::StoreSigningKey { room_key, .. } => {
            let mut key = SIGNING_KEY_PREFIX.to_vec();
            key.extend_from_slice(room_key);
            key
        }
        OrderDelegateRequestMsg::GetPublicKey { room_key } => {
            let mut key = PUBLIC_KEY_PREFIX.to_vec();
            key.extend_from_slice(room_key);
            key
        }

        // Signing operations - use prefix + room_key + request_id for uniqueness
        OrderDelegateRequestMsg::SignMessage {
            room_key,
            request_id,
            ..
        }
        | OrderDelegateRequestMsg::SignMember {
            room_key,
            request_id,
            ..
        }
        | OrderDelegateRequestMsg::SignBan {
            room_key,
            request_id,
            ..
        }
        | OrderDelegateRequestMsg::SignConfig {
            room_key,
            request_id,
            ..
        }
        | OrderDelegateRequestMsg::SignMemberInfo {
            room_key,
            request_id,
            ..
        }
        | OrderDelegateRequestMsg::SignSecretVersion {
            room_key,
            request_id,
            ..
        }
        | OrderDelegateRequestMsg::SignEncryptedSecret {
            room_key,
            request_id,
            ..
        }
        | OrderDelegateRequestMsg::SignUpgrade {
            room_key,
            request_id,
            ..
        } => {
            let mut key = SIGN_PREFIX.to_vec();
            key.extend_from_slice(room_key);
            key.extend_from_slice(&request_id.to_le_bytes());
            key
        }
    }
}

/// Send a request to the delegate and wait for the response.
pub async fn send_delegate_request(
    request: OrderDelegateRequestMsg,
) -> Result<OrderDelegateResponseMsg, String> {
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
    let contract_id = ContractInstanceId::new(DUMMY_CONTRACT_ID);
    let app_msg = ApplicationMessage::new(contract_id, payload);

    // Prepare the delegate request
    let delegate_request = DelegateOp(DelegateRequest::ApplicationMessages {
        key: delegate_key,
        params: Parameters::from(Vec::<u8>::new()),
        inbound: vec![InboundDelegateMsg::ApplicationMessage(app_msg)],
    });

    // Convert to ClientRequest and send via WebSocket
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

/// Fire a request to load data from delegate storage without waiting for response.
/// Used during initialization to avoid deadlocks in the message loop.
pub fn fire_delegate_request(request: OrderDelegateRequestMsg) {
    info!("Firing delegate request (fire and forget): {:?}", request);

    // Serialize the request
    let mut payload = Vec::new();
    if let Err(e) = into_writer(&request, &mut payload) {
        error!("Failed to serialize delegate request: {}", e);
        return;
    }

    let delegate_key = get_delegate_key();
    let contract_id = ContractInstanceId::new(DUMMY_CONTRACT_ID);
    let app_msg = ApplicationMessage::new(contract_id, payload);

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
// High-level API functions for storage operations
// ============================================================================

/// Store contract keys in the delegate.
pub async fn store_contract_keys(keys: &[String]) -> Result<(), String> {
    let mut buffer = Vec::new();
    into_writer(keys, &mut buffer)
        .map_err(|e| format!("Failed to serialize contract keys: {}", e))?;

    let request = OrderDelegateRequestMsg::StoreRequest {
        key: OrderDelegateKey::new(CONTRACT_KEYS_STORAGE_KEY.to_vec()),
        value: buffer,
    };

    match send_delegate_request(request).await? {
        OrderDelegateResponseMsg::StoreResponse { result, .. } => result,
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Load contract keys from the delegate.
pub async fn load_contract_keys() -> Result<Vec<String>, String> {
    let request = OrderDelegateRequestMsg::GetRequest {
        key: OrderDelegateKey::new(CONTRACT_KEYS_STORAGE_KEY.to_vec()),
    };

    match send_delegate_request(request).await? {
        OrderDelegateResponseMsg::GetResponse { value, .. } => {
            if let Some(data) = value {
                from_reader::<Vec<String>, _>(&data[..])
                    .map_err(|e| format!("Failed to deserialize contract keys: {}", e))
            } else {
                Ok(Vec::new())
            }
        }
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Store a signing key in the delegate.
pub async fn store_signing_key(room_key: RoomKey, signing_key_bytes: [u8; 32]) -> Result<(), String> {
    let request = OrderDelegateRequestMsg::StoreSigningKey {
        room_key,
        signing_key_bytes,
    };

    match send_delegate_request(request).await? {
        OrderDelegateResponseMsg::StoreSigningKeyResponse { result, .. } => result,
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Get the public key from a stored signing key.
pub async fn get_public_key(room_key: RoomKey) -> Result<Option<[u8; 32]>, String> {
    let request = OrderDelegateRequestMsg::GetPublicKey { room_key };

    match send_delegate_request(request).await? {
        OrderDelegateResponseMsg::GetPublicKeyResponse { public_key, .. } => Ok(public_key),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Sign a message using the signing key stored in the delegate.
pub async fn sign_message(room_key: RoomKey, message_bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    let request_id = generate_request_id();
    let request = OrderDelegateRequestMsg::SignMessage {
        room_key,
        request_id,
        message_bytes,
    };

    match send_delegate_request(request).await? {
        OrderDelegateResponseMsg::SignResponse { signature, .. } => signature,
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Fire a request to load contract keys without waiting (for initialization).
pub fn fire_load_contract_keys_request() {
    let request = OrderDelegateRequestMsg::GetRequest {
        key: OrderDelegateKey::new(CONTRACT_KEYS_STORAGE_KEY.to_vec()),
    };
    fire_delegate_request(request);
}

/// Fire a request to load signing key without waiting (for initialization).
pub fn fire_load_signing_key_request() {
    let request = OrderDelegateRequestMsg::GetRequest {
        key: OrderDelegateKey::new(SIGNING_KEY_STORAGE_KEY.to_vec()),
    };
    fire_delegate_request(request);
}
