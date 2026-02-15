use freenet_stdlib::prelude::bincode;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use dioxus::signals::{Global, GlobalSignal};
use freenet_stdlib::client_api::{
    ClientRequest, ContractRequest, ContractResponse, HostResponse, NodeDiagnosticsConfig,
    NodeQuery, QueryResponse,
};
use freenet_stdlib::prelude::tracing;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MessageEvent, WebSocket};

/// HTTP base URL for the node
pub static NODE_HTTP_BASE: GlobalSignal<String> =
    Global::new(|| "http://127.0.0.1:7509".to_string());

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub api_url: String,
}

/// Prevent duplicate polling intervals across reconnections.
static POLLING_STARTED: AtomicBool = AtomicBool::new(false);

/// Interval between diagnostics queries (milliseconds).
const POLL_INTERVAL_MS: i32 = 10_000;

// Shared WebSocket handle — replaced on each reconnection so polling closures
// always use the current connection.
thread_local! {
    static CURRENT_WS: RefCell<Option<Rc<RefCell<WebSocket>>>> = const { RefCell::new(None) };
}

fn set_current_ws(ws: Rc<RefCell<WebSocket>>) {
    CURRENT_WS.with(|cell| *cell.borrow_mut() = Some(ws));
}

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

/// Connects to the Freenet node client API and polls diagnostics periodically.
pub fn connect_node_api(config: &NodeConfig) {
    let url = config.api_url.clone();

    let ws = match WebSocket::new(&url) {
        Ok(ws) => ws,
        Err(e) => {
            tracing::error!("Failed to create WebSocket: {:?}", e);
            return;
        }
    };

    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

    let ws_rc = Rc::new(RefCell::new(ws.clone()));

    let ws_for_open = ws_rc.clone();
    let onopen = Closure::<dyn FnMut()>::new(move || {
        tracing::info!("Node API WebSocket connected");
        // Update shared handle so existing intervals use the new connection
        set_current_ws(ws_for_open.clone());

        // Send diagnostics immediately
        send_diagnostics_query(&ws_for_open.borrow());

        // Only start intervals once (they persist across reconnects)
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
        tracing::error!("Node API WebSocket error");
    });
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    let url_for_reconnect = config.api_url.clone();
    let onclose = Closure::<dyn FnMut()>::new(move || {
        tracing::warn!("Node API WebSocket closed, will reconnect in 5s");
        schedule_reconnect(url_for_reconnect.clone());
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();
}

pub fn send_request(ws: &WebSocket, request: &ClientRequest) {
    match bincode::serialize(request) {
        Ok(bytes) => {
            if let Err(e) = ws.send_with_u8_array(&bytes) {
                tracing::error!("Failed to send request: {:?}", e);
            }
        }
        Err(e) => {
            tracing::error!("Failed to serialize request: {}", e);
        }
    }
}

fn send_diagnostics_query(ws: &WebSocket) {
    let request = ClientRequest::NodeQueries(NodeQuery::NodeDiagnostics {
        config: NodeDiagnosticsConfig::full(),
    });
    send_request(ws, &request);
}

/// Parse a bincode-encoded HostResponse and update global signals.
fn handle_host_response(bytes: &[u8]) {
    use freenet_stdlib::client_api::ClientError;

    let result: Result<HostResponse, ClientError> = match bincode::deserialize(bytes) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to deserialize HostResponse: {}", e);
            return;
        }
    };

    let response = match result {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Node returned error: {:?}", e);
            return;
        }
    };

    match response {
        HostResponse::QueryResponse(QueryResponse::NodeDiagnostics(diag)) => {
            tracing::debug!("Received diagnostics: {:?}", diag);
        }
        HostResponse::ContractResponse(ContractResponse::GetResponse { key, state, .. }) => {
            tracing::debug!("Received get response for contract: {}", key);
        }
        HostResponse::ContractResponse(ContractResponse::UpdateResponse { key, .. }) => {
            tracing::debug!("Received update response for contract: {}", key);
        }
        HostResponse::ContractResponse(ContractResponse::UpdateNotification { key, .. }) => {
            tracing::debug!("Received update notification for contract: {}", key);
        }
        HostResponse::Ok => {
            tracing::debug!("Received Ok response");
        }
        _ => {
            tracing::debug!("Received unhandled response type");
        }
    }
}

/// Start polling and type-checking intervals (called exactly once).
fn start_polling_intervals() {
    // Diagnostics polling
    let diag_callback = Closure::<dyn FnMut()>::new(move || {
        with_current_ws(send_diagnostics_query);
    });
    let window = web_sys::window().expect("no global window");
    let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
        diag_callback.as_ref().unchecked_ref(),
        POLL_INTERVAL_MS,
    );
    diag_callback.forget();
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