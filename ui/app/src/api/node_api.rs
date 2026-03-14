//! Freenet Node API - WebSocket communication with the Freenet node.
//!
//! Handles WebSocket connection, contract operations (PUT, GET, SUBSCRIBE, UPDATE),
//! and response handling for contract state management.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use ciborium::{de::from_reader, ser::into_writer};
use dioxus::prelude::ReadableExt;
use dioxus::signals::{Global, GlobalSignal};
use freenet_stdlib::client_api::{
    ClientRequest, ContractRequest, ContractResponse, HostResponse, QueryResponse,
};
use freenet_stdlib::prelude::bincode;
use freenet_stdlib::prelude::tracing::{debug, error, info, warn};
use freenet_stdlib::prelude::{
    ContractCode, ContractContainer, ContractKey, ContractWasmAPIVersion, Parameters, State,
    UpdateData, WrappedContract, WrappedState,
};
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::channel::oneshot;
use pizza_common::{FullOrderStateV1, OrderParametersV1};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MessageEvent, WebSocket};

/// Contract WASM bytes - embedded at compile time
pub const CONTRACT_WASM: &[u8] =
    include_bytes!("../../../../contracts/pizza-contract/build/pizza_contract.wasm");

// ============================================================================
// Global State Signals
// ============================================================================

/// HTTP base URL for the node (derived from window.location or fallback)
pub static NODE_HTTP_BASE: GlobalSignal<String> = Global::new(|| get_http_base_url());

/// Fallback URL for non-browser environments
const FALLBACK_HTTP_URL: &str = "http://localhost:7509";

/// Get the HTTP base URL from window.location.
/// This allows the app to work on any host/port, not just localhost:7509.
fn get_http_base_url() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let location = window.location();
            let protocol = location.protocol().unwrap_or_default();
            let host = location.host().unwrap_or_default(); // includes port
            return format!("{}//{}", protocol, host);
        }
    }
    FALLBACK_HTTP_URL.to_string()
}

/// Get the WebSocket URL for connecting to the Freenet node.
/// Derives the URL from the current window.location, allowing the app to work
/// on any host/port (not just localhost:7509).
pub fn get_websocket_url() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let location = window.location();
            let protocol = location.protocol().unwrap_or_default();
            let host = location.host().unwrap_or_default(); // includes port

            let ws_protocol = if protocol == "https:" { "wss:" } else { "ws:" };
            return format!(
                "{}//{}/v1/contract/command?encodingProtocol=native",
                ws_protocol, host
            );
        }
    }
    "ws://localhost:7509/v1/contract/command?encodingProtocol=native".to_string()
}

/// Authorization token from the Freenet gateway
pub static AUTH_TOKEN: GlobalSignal<Option<String>> = Global::new(|| None);

/// Connection status
#[derive(Clone, Debug, PartialEq, Default)]
pub enum ConnectionStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

pub static CONNECTION_STATUS: GlobalSignal<ConnectionStatus> =
    Global::new(|| ConnectionStatus::Disconnected);

/// Signal indicating when the delegate is ready to receive requests
pub static DELEGATE_READY: GlobalSignal<bool> = Global::new(|| false);

/// Stored contracts - maps contract key string to (state, parameters, ContractKey)
pub static CONTRACTS: GlobalSignal<
    HashMap<String, (FullOrderStateV1, OrderParametersV1, ContractKey)>,
> = Global::new(HashMap::new);

// ============================================================================
// Response Types
// ============================================================================

/// Response from a PUT (publish contract) operation
#[derive(Clone, Debug)]
pub struct PutResponse {
    pub contract_key: String,
    pub success: bool,
}

/// Response from an UPDATE operation
#[derive(Clone, Debug)]
pub struct UpdateResponse {
    pub contract_key: String,
    pub success: bool,
}

/// Response from a GET operation
#[derive(Clone, Debug)]
pub struct GetResponse {
    pub contract_key: String,
    pub state: Option<FullOrderStateV1>,
}

/// Response from a SUBSCRIBE operation
#[derive(Clone, Debug)]
pub struct SubscribeResponse {
    pub contract_key: String,
    pub subscribed: bool,
}

// ============================================================================
// Pending Request Tracking
// ============================================================================

thread_local! {
    /// Pending PUT requests - waiting for PutResponse
    static PENDING_PUT: RefCell<HashMap<String, oneshot::Sender<PutResponse>>> =
        RefCell::new(HashMap::new());

    /// Pending UPDATE requests - waiting for UpdateResponse
    static PENDING_UPDATE: RefCell<HashMap<String, oneshot::Sender<UpdateResponse>>> =
        RefCell::new(HashMap::new());

    /// Pending GET requests - waiting for GetResponse
    static PENDING_GET: RefCell<HashMap<String, oneshot::Sender<GetResponse>>> =
        RefCell::new(HashMap::new());

    /// Pending SUBSCRIBE requests - waiting for SubscribeResponse
    static PENDING_SUBSCRIBE: RefCell<HashMap<String, oneshot::Sender<SubscribeResponse>>> =
        RefCell::new(HashMap::new());

    /// Contract update subscribers
    static CONTRACT_UPDATE_SENDERS: RefCell<HashMap<String, Vec<UnboundedSender<(FullOrderStateV1, OrderParametersV1)>>>> =
        RefCell::new(HashMap::new());

    /// Contract list subscribers
    static CONTRACT_LIST_SENDERS: RefCell<Vec<UnboundedSender<Vec<String>>>> =
        RefCell::new(Vec::new());

    /// Tracks contracts we have subscribed to (for re-subscription on reconnect)
    static SUBSCRIBED_CONTRACTS: RefCell<std::collections::HashSet<String>> =
        RefCell::new(std::collections::HashSet::new());
}

/// Register a pending PUT request and return a receiver for the response
fn register_pending_put(contract_key: &str) -> oneshot::Receiver<PutResponse> {
    let (tx, rx) = oneshot::channel();
    PENDING_PUT.with(|pending| {
        pending.borrow_mut().insert(contract_key.to_string(), tx);
    });
    rx
}

/// Register a pending UPDATE request and return a receiver for the response
fn register_pending_update(contract_key: &str) -> oneshot::Receiver<UpdateResponse> {
    let (tx, rx) = oneshot::channel();
    PENDING_UPDATE.with(|pending| {
        pending.borrow_mut().insert(contract_key.to_string(), tx);
    });
    rx
}

/// Register a pending GET request and return a receiver for the response
fn register_pending_get(contract_key: &str) -> oneshot::Receiver<GetResponse> {
    let (tx, rx) = oneshot::channel();
    PENDING_GET.with(|pending| {
        pending.borrow_mut().insert(contract_key.to_string(), tx);
    });
    rx
}

/// Register a pending SUBSCRIBE request and return a receiver for the response
fn register_pending_subscribe(contract_key: &str) -> oneshot::Receiver<SubscribeResponse> {
    let (tx, rx) = oneshot::channel();
    PENDING_SUBSCRIBE.with(|pending| {
        pending.borrow_mut().insert(contract_key.to_string(), tx);
    });
    rx
}

// ============================================================================
// Stream Subscriptions
// ============================================================================

/// Subscribe to updates for a specific contract.
/// Immediately emits the current state if available, then emits on changes.
pub fn subscribe_to_contract_updates(
    contract_key: &str,
) -> UnboundedReceiver<(FullOrderStateV1, OrderParametersV1)> {
    let (tx, rx) = unbounded();

    // Emit current state immediately if available
    {
        let contracts = CONTRACTS.read();
        if let Some((state, params, _)) = contracts.get(contract_key) {
            let _ = tx.unbounded_send((state.clone(), params.clone()));
        }
    }

    CONTRACT_UPDATE_SENDERS.with(|senders| {
        let mut senders = senders.borrow_mut();
        senders
            .entry(contract_key.to_string())
            .or_insert_with(Vec::new)
            .push(tx);
    });
    rx
}

/// Subscribe to contract list changes.
/// Immediately emits the current contract list, then emits on changes.
pub fn subscribe_to_contract_list() -> UnboundedReceiver<Vec<String>> {
    let (tx, rx) = unbounded();

    // Emit current contract list immediately
    let current_keys: Vec<String> = {
        let contracts = CONTRACTS.read();
        contracts.keys().cloned().collect()
    };
    let _ = tx.unbounded_send(current_keys);

    CONTRACT_LIST_SENDERS.with(|senders| {
        senders.borrow_mut().push(tx);
    });
    rx
}

/// Notify subscribers of contract update
pub fn notify_contract_update(key: &str, state: &FullOrderStateV1, params: &OrderParametersV1) {
    CONTRACT_UPDATE_SENDERS.with(|senders| {
        let mut senders = senders.borrow_mut();
        if let Some(list) = senders.get_mut(key) {
            list.retain(|sender| {
                sender
                    .unbounded_send((state.clone(), params.clone()))
                    .is_ok()
            });
        }
    });
}

/// Notify subscribers of contract list change
pub fn notify_contract_list_change(contracts: Vec<String>) {
    CONTRACT_LIST_SENDERS.with(|senders| {
        let mut senders = senders.borrow_mut();
        senders.retain(|sender| sender.unbounded_send(contracts.clone()).is_ok());
    });
}

// ============================================================================
// WebSocket Connection
// ============================================================================

thread_local! {
    static CURRENT_WS: RefCell<Option<Rc<RefCell<WebSocket>>>> = const { RefCell::new(None) };
    /// Queue of serialized requests to send once the WebSocket connection is established
    pub static PENDING_SEND_QUEUE: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

fn set_current_ws(ws: Rc<RefCell<WebSocket>>) {
    CURRENT_WS.with(|cell| *cell.borrow_mut() = Some(ws));
}

/// Execute a function with the current WebSocket if connected
pub fn with_current_ws<F: FnOnce(&WebSocket)>(f: F) {
    CURRENT_WS.with(|cell| {
        if let Some(ws) = cell.borrow().as_ref() {
            let ws = ws.borrow();
            if ws.ready_state() == WebSocket::OPEN {
                f(&ws);
            }
        }
    });
}

/// Check if WebSocket is connected
pub fn is_connected() -> bool {
    let mut connected = false;
    CURRENT_WS.with(|cell| {
        if let Some(ws) = cell.borrow().as_ref() {
            connected = ws.borrow().ready_state() == WebSocket::OPEN;
        }
    });
    connected
}

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub api_url: String,
}

const POLL_INTERVAL_MS: i32 = 30_000;
static POLLING_STARTED: AtomicBool = AtomicBool::new(false);

/// Gets the authorization token from the window global variable.
pub fn get_auth_token_from_window() {
    if let Some(win) = web_sys::window() {
        match js_sys::Reflect::get(&win, &"__FREENET_AUTH_TOKEN__".into()) {
            Ok(token_value) => {
                let location = win.location();
                let host = location.host().unwrap_or_default();
                let protocol = location.protocol().unwrap_or_default();

                if let Some(token) = token_value.as_string() {
                    info!("Found auth token from window global");
                    *AUTH_TOKEN.write() = Some(token);
                    *NODE_HTTP_BASE.write() = format!("{}//{}", protocol, host)
                } else if token_value.is_undefined() || token_value.is_null() {
                    debug!("Auth token not injected by gateway (running locally?)");
                    *NODE_HTTP_BASE.write() = "http://127.0.0.1:7509".into();
                } else {
                    debug!("Auth token has unexpected type");
                }
            }
            Err(err) => {
                error!("Failed to read auth token from window: {:?}", err);
            }
        }
    }
}

/// Connects to the Freenet node client API
pub fn connect_node_api(config: &NodeConfig) {
    *CONNECTION_STATUS.write() = ConnectionStatus::Connecting;

    let mut url = config.api_url.clone();

    if let Some(ref token) = *AUTH_TOKEN.read() {
        if url.contains('?') {
            url.push_str(&format!("&authToken={}", token));
        } else {
            url.push_str(&format!("?authToken={}", token));
        }
    }

    let ws = match WebSocket::new(&url) {
        Ok(ws) => ws,
        Err(e) => {
            error!("Failed to create WebSocket: {:?}", e);
            *CONNECTION_STATUS.write() = ConnectionStatus::Error(format!("{:?}", e));
            return;
        }
    };

    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

    let ws_rc = Rc::new(RefCell::new(ws.clone()));

    let ws_for_open = ws_rc.clone();
    let onopen = Closure::<dyn FnMut()>::new(move || {
        info!("Node API WebSocket connected");
        *CONNECTION_STATUS.write() = ConnectionStatus::Connected;
        set_current_ws(ws_for_open.clone());

        // Register the pizza delegate first
        super::delegate_api::register_delegate();

        // Spawn a task to mark delegate as ready after a short delay
        // This gives the node time to actually register the delegate
        wasm_bindgen_futures::spawn_local(async {
            // Wait 500ms for delegate registration to complete
            let promise = js_sys::Promise::new(&mut |resolve, _| {
                web_sys::window()
                    .unwrap()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 500)
                    .unwrap();
            });
            wasm_bindgen_futures::JsFuture::from(promise).await.ok();

            info!("Delegate ready, loading contracts");
            *DELEGATE_READY.write() = true;

            // Load contract keys from delegate and fetch contracts from network
            match crate::services::freenet::FreenetService::load_contracts_from_delegate().await {
                Ok(keys) => info!("Loaded {} contracts from delegate", keys.len()),
                Err(e) => warn!("Failed to load contracts from delegate: {}", e),
            }
        });

        // Flush any contract requests that were queued while connecting
        // (but NOT delegate requests - those should wait for delegate to be ready)
        flush_pending_requests(&ws_for_open.borrow());

        // Re-subscribe to all tracked subscriptions
        let subscribed_keys: Vec<String> =
            SUBSCRIBED_CONTRACTS.with(|subs| subs.borrow().iter().cloned().collect());

        let contracts = CONTRACTS.read();
        for key_str in subscribed_keys {
            if let Some((_, _, contract_key)) = contracts.get(&key_str) {
                let request = ClientRequest::ContractOp(ContractRequest::Subscribe {
                    key: contract_key.clone().into(),
                    summary: None,
                });
                send_request(&ws_for_open.borrow(), &request);
                info!("Re-subscribing to contract on reconnect: {}", key_str);
            }
        }

        if !POLLING_STARTED.swap(true, Ordering::SeqCst) {
            start_polling_intervals();
        }
    });
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        let data = e.data();
        if let Ok(abuf) = data.dyn_into::<js_sys::ArrayBuffer>() {
            let bytes = js_sys::Uint8Array::new(&abuf).to_vec();
            handle_host_response(&bytes);
        }
    });
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let onerror = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
        error!("Node API WebSocket error");
        *CONNECTION_STATUS.write() = ConnectionStatus::Error("WebSocket error".into());
    });
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    let url_for_reconnect = config.api_url.clone();
    let onclose = Closure::<dyn FnMut()>::new(move || {
        warn!("Node API WebSocket closed, will reconnect in 5s");
        *CONNECTION_STATUS.write() = ConnectionStatus::Disconnected;
        schedule_reconnect(url_for_reconnect.clone());
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();
}

/// Send a request to the Freenet node
pub fn send_request(ws: &WebSocket, request: &ClientRequest) {
    match bincode::serialize(request) {
        Ok(bytes) => {
            if let Err(e) = ws.send_with_u8_array(&bytes) {
                error!("Failed to send request: {:?}", e);
            }
        }
        Err(e) => {
            error!("Failed to serialize request: {}", e);
        }
    }
}

/// Send a request using the current WebSocket connection.
/// If the connection is not yet established, the request is queued and will be sent
/// once the connection is ready.
pub fn send_request_current(request: &ClientRequest) {
    let sent = CURRENT_WS.with(|cell| {
        if let Some(ws) = cell.borrow().as_ref() {
            let ws = ws.borrow();
            if ws.ready_state() == WebSocket::OPEN {
                send_request(&ws, request);
                return true;
            }
        }
        false
    });

    if !sent {
        // Serialize and queue the request to be sent when connection is established
        match bincode::serialize(request) {
            Ok(bytes) => {
                PENDING_SEND_QUEUE.with(|queue| {
                    queue.borrow_mut().push(bytes);
                });
                debug!("Request queued (WebSocket not ready)");
            }
            Err(e) => {
                error!("Failed to serialize request for queue: {}", e);
            }
        }
    }
}

/// Flush all queued requests (called when WebSocket connection opens)
fn flush_pending_requests(ws: &WebSocket) {
    PENDING_SEND_QUEUE.with(|queue| {
        let requests: Vec<Vec<u8>> = queue.borrow_mut().drain(..).collect();
        if !requests.is_empty() {
            info!("Flushing {} queued requests", requests.len());
            for bytes in requests {
                if let Err(e) = ws.send_with_u8_array(&bytes) {
                    error!("Failed to send queued request: {:?}", e);
                }
            }
        }
    });
}

fn start_polling_intervals() {
    let callback = Closure::<dyn FnMut()>::new(move || {
        debug!("Polling interval tick");
    });
    let window = web_sys::window().expect("no global window");
    let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
        callback.as_ref().unchecked_ref(),
        POLL_INTERVAL_MS,
    );
    callback.forget();
}

fn schedule_reconnect(url: String) {
    let callback = Closure::<dyn FnMut()>::new(move || {
        let config = NodeConfig {
            api_url: url.clone(),
        };
        connect_node_api(&config);
    });

    let window = web_sys::window().expect("no global window");
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        callback.as_ref().unchecked_ref(),
        5_000,
    );
    callback.forget();
}

// ============================================================================
// Response Handling
// ============================================================================

fn handle_host_response(bytes: &[u8]) {
    use freenet_stdlib::client_api::ClientError;

    let result: Result<HostResponse, ClientError> = match bincode::deserialize(bytes) {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to deserialize HostResponse: {}", e);
            return;
        }
    };

    let response = match result {
        Ok(r) => r,
        Err(e) => {
            warn!("Node returned error: {:?}", e);
            return;
        }
    };

    match response {
        HostResponse::ContractResponse(contract_response) => {
            handle_contract_response(contract_response);
        }
        HostResponse::DelegateResponse { key: _, values } => {
            handle_delegate_response(values);
        }
        HostResponse::QueryResponse(QueryResponse::NodeDiagnostics(diag)) => {
            debug!("Received diagnostics: {:?}", diag);
        }
        HostResponse::Ok => {
            debug!("Received Ok response");
        }
        _ => {
            debug!("Received unhandled response type");
        }
    }
}

fn handle_contract_response(response: ContractResponse) {
    match response {
        ContractResponse::GetResponse {
            key,
            contract,
            state,
        } => {
            handle_get_response(key, contract, state);
        }
        ContractResponse::PutResponse { key } => {
            handle_put_response(key);
        }
        ContractResponse::UpdateResponse { key, summary: _ } => {
            handle_update_response(key);
        }
        ContractResponse::UpdateNotification { key, update } => {
            handle_update_notification(key, update);
        }
        ContractResponse::SubscribeResponse { key, subscribed } => {
            handle_subscribe_response(key, subscribed);
        }
        _ => {
            debug!("Received unhandled contract response type");
        }
    }
}

fn handle_get_response(key: ContractKey, contract: Option<ContractContainer>, state: WrappedState) {
    let key_str = key.to_string();
    info!("Received GetResponse for contract: {}", key_str);

    let state_bytes = state.as_ref();
    if state_bytes.is_empty() {
        debug!("Empty state received for {}", key_str);
        // Resolve pending GET with None
        PENDING_GET.with(|pending| {
            if let Some(sender) = pending.borrow_mut().remove(&key_str) {
                let _ = sender.send(GetResponse {
                    contract_key: key_str.clone(),
                    state: None,
                });
            }
        });
        return;
    }

    match from_reader::<FullOrderStateV1, &[u8]>(state_bytes) {
        Ok(network_state) => {
            let mut contracts = CONTRACTS.write();

            // Try to get existing state and params
            let existing = contracts.get(&key_str).cloned();

            // Get params from existing state or extract from contract container
            let params_opt: Option<OrderParametersV1> = if let Some((_, params, _)) = &existing {
                Some(params.clone())
            } else if let Some(ref container) = contract {
                // Extract parameters from the contract container
                let params = container.params();
                let params_bytes = params.as_ref();
                from_reader::<OrderParametersV1, &[u8]>(params_bytes).ok()
            } else {
                None
            };

            if let Some(params) = params_opt {
                let is_new = existing.is_none();

                // If we have existing state, merge instead of overwrite
                // This preserves optimistic updates that haven't propagated to the network yet
                let final_state = if let Some((current_state, _, _)) = &existing {
                    let mut merged = current_state.clone();
                    if let Err(e) = pizza_common::ComposableState::merge(
                        &mut merged,
                        current_state,
                        &params,
                        &network_state,
                    ) {
                        error!("Failed to merge state in GetResponse: {}", e);
                        // Fall back to network state on merge error
                        network_state.clone()
                    } else {
                        merged
                    }
                } else {
                    network_state.clone()
                };

                contracts.insert(
                    key_str.clone(),
                    (final_state.clone(), params.clone(), key.clone()),
                );
                drop(contracts);

                notify_contract_update(&key_str, &final_state, &params);

                // If this is a new contract, notify contract list change
                if is_new {
                    let contracts = CONTRACTS.read();
                    let keys: Vec<String> = contracts.keys().cloned().collect();
                    notify_contract_list_change(keys);
                }

                // Resolve pending GET
                PENDING_GET.with(|pending| {
                    if let Some(sender) = pending.borrow_mut().remove(&key_str) {
                        let _ = sender.send(GetResponse {
                            contract_key: key_str.clone(),
                            state: Some(final_state),
                        });
                    }
                });
            } else {
                drop(contracts);
                warn!("Got state but no parameters for {}", key_str);
                // Still resolve pending GET with None since we can't use the state without params
                PENDING_GET.with(|pending| {
                    if let Some(sender) = pending.borrow_mut().remove(&key_str) {
                        let _ = sender.send(GetResponse {
                            contract_key: key_str.clone(),
                            state: None,
                        });
                    }
                });
            }
        }
        Err(e) => {
            error!("Failed to deserialize state: {}", e);
            // Resolve pending GET with None on error
            PENDING_GET.with(|pending| {
                if let Some(sender) = pending.borrow_mut().remove(&key_str) {
                    let _ = sender.send(GetResponse {
                        contract_key: key_str.clone(),
                        state: None,
                    });
                }
            });
        }
    }
}

fn handle_put_response(key: ContractKey) {
    let key_str = key.to_string();
    info!("Contract published successfully: {}", key_str);

    // Resolve pending PUT request
    PENDING_PUT.with(|pending| {
        if let Some(sender) = pending.borrow_mut().remove(&key_str) {
            let _ = sender.send(PutResponse {
                contract_key: key_str.clone(),
                success: true,
            });
        }
    });

    // Subscribe to updates for this contract
    let request = ClientRequest::ContractOp(ContractRequest::Subscribe {
        key: key.into(),
        summary: None,
    });
    send_request_current(&request);

    // Notify contract list change
    let contracts = CONTRACTS.read();
    let keys: Vec<String> = contracts.keys().cloned().collect();
    notify_contract_list_change(keys);
}

fn handle_update_response(key: ContractKey) {
    let key_str = key.to_string();
    info!("Update acknowledged for contract: {}", key_str);

    // Resolve pending UPDATE request
    PENDING_UPDATE.with(|pending| {
        if let Some(sender) = pending.borrow_mut().remove(&key_str) {
            let _ = sender.send(UpdateResponse {
                contract_key: key_str.clone(),
                success: true,
            });
        }
    });

    // Notify subscribers of the confirmed update
    let contracts = CONTRACTS.read();
    if let Some((state, params, _)) = contracts.get(&key_str) {
        notify_contract_update(&key_str, state, params);
    }
}

fn handle_update_notification(key: ContractKey, update: UpdateData<'static>) {
    let key_str = key.to_string();
    info!("Received update notification for contract: {}", key_str);

    let mut contracts = CONTRACTS.write();
    if let Some((current_state, params, contract_key)) = contracts.get(&key_str).cloned() {
        match update {
            UpdateData::State(new_state_bytes) => {
                match from_reader::<FullOrderStateV1, &[u8]>(new_state_bytes.as_ref()) {
                    Ok(new_state) => {
                        let mut merged = current_state.clone();
                        if let Err(e) = pizza_common::ComposableState::merge(
                            &mut merged,
                            &current_state,
                            &params,
                            &new_state,
                        ) {
                            error!("Failed to merge state: {}", e);
                            return;
                        }
                        contracts.insert(
                            key_str.clone(),
                            (merged.clone(), params.clone(), contract_key),
                        );
                        drop(contracts);
                        notify_contract_update(&key_str, &merged, &params);
                    }
                    Err(e) => {
                        error!("Failed to deserialize update state: {}", e);
                    }
                }
            }
            UpdateData::Delta(delta_bytes) => {
                if delta_bytes.as_ref().is_empty() {
                    return;
                }
                match from_reader::<pizza_common::FullOrderStateV1Delta, &[u8]>(
                    delta_bytes.as_ref(),
                ) {
                    Ok(delta) => {
                        let mut new_state = current_state.clone();
                        if let Err(e) = pizza_common::ComposableState::apply_delta(
                            &mut new_state,
                            &current_state,
                            &params,
                            &Some(delta),
                        ) {
                            error!("Failed to apply delta: {}", e);
                            return;
                        }
                        contracts.insert(
                            key_str.clone(),
                            (new_state.clone(), params.clone(), contract_key),
                        );
                        drop(contracts);
                        notify_contract_update(&key_str, &new_state, &params);
                    }
                    Err(e) => {
                        error!("Failed to deserialize delta: {}", e);
                    }
                }
            }
            _ => {
                debug!("Unhandled update data type");
            }
        }
    } else {
        drop(contracts);
        let request = ClientRequest::ContractOp(ContractRequest::Get {
            key: key.clone().into(),
            return_contract_code: false,
            subscribe: true,
            blocking_subscribe: false,
        });
        send_request_current(&request);
    }
}

fn handle_subscribe_response(key: ContractKey, subscribed: bool) {
    let key_str = key.to_string();

    // Resolve pending SUBSCRIBE request
    PENDING_SUBSCRIBE.with(|pending| {
        if let Some(sender) = pending.borrow_mut().remove(&key_str) {
            let _ = sender.send(SubscribeResponse {
                contract_key: key_str.clone(),
                subscribed,
            });
        }
    });

    if subscribed {
        info!("Subscribed to contract: {}", key_str);
        let request = ClientRequest::ContractOp(ContractRequest::Get {
            key: key.into(),
            return_contract_code: false,
            subscribe: false,
            blocking_subscribe: false,
        });
        send_request_current(&request);
    } else {
        warn!("Failed to subscribe to contract: {}", key_str);
    }
}

fn handle_delegate_response(values: Vec<freenet_stdlib::prelude::OutboundDelegateMsg>) {
    use freenet_stdlib::prelude::OutboundDelegateMsg;
    use pizza_common::order_delegate::PizzaDelegateResponse;

    info!(
        "Received delegate response containing {} values",
        values.len()
    );

    for (i, v) in values.iter().enumerate() {
        debug!("Processing delegate response value #{}", i);
        match v {
            OutboundDelegateMsg::ApplicationMessage(app_msg) => {
                debug!(
                    "Delegate response is an ApplicationMessage, processed flag: {}",
                    app_msg.processed
                );

                // Try to deserialize as a response
                match from_reader::<PizzaDelegateResponse, _>(app_msg.payload.as_slice()) {
                    Ok(response) => {
                        info!(
                            "Successfully deserialized PizzaDelegateResponse: {:?}",
                            response
                        );

                        // Try to complete any pending request waiting for this response
                        let completed = match &response {
                            PizzaDelegateResponse::StoreContractKeysResponse { .. }
                            | PizzaDelegateResponse::GetContractKeysResponse { .. } => {
                                super::delegate_api::complete_pending_contract_keys_request(
                                    response.clone(),
                                )
                            }
                            PizzaDelegateResponse::StoreSigningKeyResponse { .. }
                            | PizzaDelegateResponse::GetSigningKeyResponse { .. } => {
                                super::delegate_api::complete_pending_signing_key_request(
                                    response.clone(),
                                )
                            }
                            PizzaDelegateResponse::GetPublicKeyResponse { .. } => {
                                super::delegate_api::complete_pending_public_key_request(
                                    response.clone(),
                                )
                            }
                            PizzaDelegateResponse::SignResponse { request_id, .. } => {
                                super::delegate_api::complete_pending_sign_request(
                                    *request_id,
                                    response.clone(),
                                )
                            }
                        };

                        if completed {
                            info!("Completed pending delegate request");
                        }

                        // Log based on response type
                        match response {
                            PizzaDelegateResponse::StoreContractKeysResponse { result } => {
                                match result {
                                    Ok(_) => info!("Successfully stored contract keys"),
                                    Err(e) => warn!("Failed to store contract keys: {}", e),
                                }
                            }
                            PizzaDelegateResponse::GetContractKeysResponse { keys } => {
                                info!("Got {} contract keys from delegate", keys.len());
                            }
                            PizzaDelegateResponse::StoreSigningKeyResponse { result } => {
                                match result {
                                    Ok(_) => info!("Successfully stored signing key"),
                                    Err(e) => warn!("Failed to store signing key: {}", e),
                                }
                            }
                            PizzaDelegateResponse::GetSigningKeyResponse { signing_key } => {
                                info!("Got signing key: present={}", signing_key.is_some());
                            }
                            PizzaDelegateResponse::GetPublicKeyResponse { public_key } => {
                                info!("Got public key: present={}", public_key.is_some());
                            }
                            PizzaDelegateResponse::SignResponse {
                                request_id,
                                signature,
                            } => match signature {
                                Ok(_) => info!("Got signature for request_id={}", request_id),
                                Err(e) => {
                                    warn!("Failed to sign request_id={}: {}", request_id, e)
                                }
                            },
                        }
                    }
                    Err(e) => {
                        warn!("Failed to deserialize delegate response: {}", e);
                    }
                }
            }
            _ => {
                warn!("Unhandled delegate response type: {:?}", v);
            }
        }
    }
}

// ============================================================================
// Contract Operations (Async)
// ============================================================================

pub fn to_cbor_vec<T: serde::Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    into_writer(value, &mut bytes).expect("CBOR serialization failed");
    bytes
}

/// Generate a contract key from parameters
pub fn generate_contract_key(params: &OrderParametersV1) -> ContractKey {
    let params_bytes = to_cbor_vec(params);
    let code = ContractCode::from(CONTRACT_WASM.to_vec());
    let params_obj = Parameters::from(params_bytes);
    ContractKey::from_params_and_code(&params_obj, &code)
}

/// Publish a new contract and return a future that resolves when acknowledged.
///
/// Returns the contract key string and a receiver for the response.
pub fn publish_contract_async(
    state: &FullOrderStateV1,
    params: &OrderParametersV1,
) -> (String, oneshot::Receiver<PutResponse>) {
    let state_bytes = to_cbor_vec(state);
    let params_bytes = to_cbor_vec(params);

    let code = ContractCode::from(CONTRACT_WASM.to_vec());
    let params_obj = Parameters::from(params_bytes);
    let contract_key = ContractKey::from_params_and_code(&params_obj, &code);
    let key_str = contract_key.to_string();

    // Register pending request before sending
    let response_rx = register_pending_put(&key_str);

    // Store locally first
    {
        let mut contracts = CONTRACTS.write();
        contracts.insert(
            key_str.clone(),
            (state.clone(), params.clone(), contract_key.clone()),
        );
    }

    // Create the contract container
    let contract = ContractContainer::Wasm(ContractWasmAPIVersion::V1(WrappedContract::new(
        code.into(),
        params_obj,
    )));

    let wrapped_state = WrappedState::new(state_bytes.into());

    let request = ClientRequest::ContractOp(ContractRequest::Put {
        contract,
        state: wrapped_state,
        related_contracts: Default::default(),
        subscribe: true,
        blocking_subscribe: false,
    });

    send_request_current(&request);
    info!("Publishing contract: {}", key_str);

    // Track this subscription for re-subscription on reconnect (PUT with subscribe=true)
    SUBSCRIBED_CONTRACTS.with(|subs| {
        subs.borrow_mut().insert(key_str.clone());
    });

    // Notify contract list change
    let contracts = CONTRACTS.read();
    let keys: Vec<String> = contracts.keys().cloned().collect();
    notify_contract_list_change(keys);

    (key_str, response_rx)
}

/// Subscribe to an existing contract and return a future that resolves when subscribed.
pub fn subscribe_to_contract_async(
    contract_key_str: &str,
) -> Option<oneshot::Receiver<SubscribeResponse>> {
    let contracts = CONTRACTS.read();
    if let Some((_, _, contract_key)) = contracts.get(contract_key_str) {
        let response_rx = register_pending_subscribe(contract_key_str);

        // Track this subscription for re-subscription on reconnect
        SUBSCRIBED_CONTRACTS.with(|subs| {
            subs.borrow_mut().insert(contract_key_str.to_string());
        });

        let request = ClientRequest::ContractOp(ContractRequest::Subscribe {
            key: contract_key.clone().into(),
            summary: None,
        });
        drop(contracts);
        send_request_current(&request);
        info!("Subscribing to contract: {}", contract_key_str);
        Some(response_rx)
    } else {
        error!("Cannot subscribe to unknown contract: {}", contract_key_str);
        None
    }
}

/// Send an update to a contract and return a future that resolves when acknowledged.
pub fn send_contract_update_async(
    contract_key_str: &str,
    state: &FullOrderStateV1,
) -> Option<oneshot::Receiver<UpdateResponse>> {
    let contracts = CONTRACTS.read();
    if let Some((_, _, contract_key)) = contracts.get(contract_key_str) {
        let contract_key = contract_key.clone();
        drop(contracts);

        // Register pending request
        let response_rx = register_pending_update(contract_key_str);

        let state_bytes = to_cbor_vec(state);
        let request = ClientRequest::ContractOp(ContractRequest::Update {
            key: contract_key.into(),
            data: UpdateData::State(State::from(state_bytes).into()),
        });
        send_request_current(&request);
        info!("Sending update to contract: {}", contract_key_str);
        Some(response_rx)
    } else {
        error!("Invalid contract key: {}", contract_key_str);
        None
    }
}

/// Send a delta update to a contract and return a future that resolves when acknowledged.
pub fn send_contract_delta_async(
    contract_key_str: &str,
    delta: &pizza_common::FullOrderStateV1Delta,
) -> Option<oneshot::Receiver<UpdateResponse>> {
    let contracts = CONTRACTS.read();
    if let Some((_, _, ref key)) = contracts.get(contract_key_str) {
        let contract_key: ContractKey = key.clone();
        drop(contracts);

        // Register pending request
        let response_rx = register_pending_update(contract_key_str);

        let delta_bytes = to_cbor_vec(delta);
        let request = ClientRequest::ContractOp(ContractRequest::Update {
            key: contract_key.into(),
            data: UpdateData::Delta(freenet_stdlib::prelude::StateDelta::from(delta_bytes).into()),
        });
        send_request_current(&request);
        info!("Sending delta to contract: {}", contract_key_str);
        Some(response_rx)
    } else {
        error!("Invalid contract key: {}", contract_key_str);
        None
    }
}

// ============================================================================
// Synchronous helpers (for reading cached state)
// ============================================================================

/// Get contract from local cache (synchronous, reads from cache only)
pub fn get_contract_state_cached(
    contract_key: &str,
) -> Option<(FullOrderStateV1, OrderParametersV1)> {
    let contracts = CONTRACTS.read();
    contracts.get(contract_key).map(
        |(state, params, _): &(FullOrderStateV1, OrderParametersV1, ContractKey)| {
            (state.clone(), params.clone())
        },
    )
}

/// Get all contract keys (synchronous, reads from cache)
pub fn get_contract_keys() -> Vec<String> {
    let contracts = CONTRACTS.read();
    contracts.keys().cloned().collect()
}

// ============================================================================
// Async Contract Fetching (from network)
// ============================================================================

/// Fetch contract state from Freenet via GET request.
///
/// This sends a GET request to the network and returns a future that resolves
/// when the state is received. The state is also cached in CONTRACTS.
pub fn get_contract_state_async(
    contract_key_str: &str,
    params: &OrderParametersV1,
) -> oneshot::Receiver<GetResponse> {
    // Compute the actual ContractKey from parameters
    let params_bytes = to_cbor_vec(params);
    let code = ContractCode::from(CONTRACT_WASM.to_vec());
    let params_obj = Parameters::from(params_bytes);
    let contract_key = ContractKey::from_params_and_code(&params_obj, &code);

    // Store parameters in cache (state will be filled when response arrives)
    {
        let mut contracts = CONTRACTS.write();
        if !contracts.contains_key(contract_key_str) {
            contracts.insert(
                contract_key_str.to_string(),
                (
                    FullOrderStateV1::default(),
                    params.clone(),
                    contract_key.clone(),
                ),
            );
        }
    }

    // Register pending request
    let response_rx = register_pending_get(contract_key_str);

    // Send GET request
    let request = ClientRequest::ContractOp(ContractRequest::Get {
        key: contract_key.into(),
        return_contract_code: false,
        subscribe: true,
        blocking_subscribe: false,
    });
    send_request_current(&request);
    info!("Requesting contract state: {}", contract_key_str);

    response_rx
}

/// Fetch contract state by key string only (for contracts already in cache with params).
/// Returns None if the contract key is not known.
pub fn get_contract_by_key_async(contract_key_str: &str) -> Option<oneshot::Receiver<GetResponse>> {
    let contracts = CONTRACTS.read();
    if let Some((_, _, contract_key)) = contracts.get(contract_key_str) {
        let contract_key = contract_key.clone();
        drop(contracts);

        // Register pending request
        let response_rx = register_pending_get(contract_key_str);

        // Send GET request
        let request = ClientRequest::ContractOp(ContractRequest::Get {
            key: contract_key.into(),
            return_contract_code: false,
            subscribe: true,
            blocking_subscribe: false,
        });
        send_request_current(&request);
        info!("Requesting contract state by key: {}", contract_key_str);

        Some(response_rx)
    } else {
        None
    }
}

/// Fetch an unknown contract by its key string.
/// This requests the contract code to get the parameters.
/// Use this when visiting an order page for a contract we don't know about.
pub fn fetch_unknown_contract_async(contract_key_str: &str) -> oneshot::Receiver<GetResponse> {
    use freenet_stdlib::prelude::ContractInstanceId;
    use std::str::FromStr;

    // Parse the contract key string to ContractInstanceId
    let contract_id =
        ContractInstanceId::from_str(contract_key_str).expect("Invalid contract key string");

    // Register pending request
    let response_rx = register_pending_get(contract_key_str);

    // Send GET request with return_contract_code: true to get parameters
    let request = ClientRequest::ContractOp(ContractRequest::Get {
        key: contract_id,
        return_contract_code: true,
        subscribe: true,
        blocking_subscribe: false,
    });
    send_request_current(&request);
    info!("Fetching unknown contract: {}", contract_key_str);

    response_rx
}
