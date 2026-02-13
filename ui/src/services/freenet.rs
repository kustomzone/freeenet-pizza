//! Freenet Communication Service
//!
//! Handles WebSocket connection to the Freenet node and manages
//! contract and delegate interactions.

use dioxus::prelude::*;
use pizza_common::order_delegate::{OrderDelegateRequestMsg, OrderDelegateResponseMsg};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::{MessageEvent, WebSocket};

/// Client Request to Freenet Node (JSON-RPC)
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientRequest {
    Put {
        contract: ContractContainer,
        state: WrappedState,
    },
    Update {
        key: ContractKey,
        delta: WrappedDelta,
    },
    Subscribe {
        key: ContractKey,
    },
    Application {
        delegate: DelegateKey,
        message: Vec<u8>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractContainer {
    pub data: Vec<u8>,
    pub parameters: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WrappedState(pub Vec<u8>);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WrappedDelta(pub Vec<u8>);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContractKey(pub String);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DelegateKey(pub String);

/// Host Response from Freenet Node
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HostResponse {
    PutResponse {
        key: ContractKey,
    },
    UpdateResponse {
        key: ContractKey,
        summary: Vec<u8>,
    },
    ContractUpdate {
        key: ContractKey,
        state: WrappedState,
    },
    DelegateResponse {
        key: DelegateKey,
        message: Vec<u8>,
    },
    Err {
        error: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct FreenetService {
    ws: WebSocket,
}

impl FreenetService {
    pub fn new(url: &str, on_msg: EventHandler<HostResponse>) -> Result<Self, JsValue> {
        let ws = WebSocket::new(url)?;

        let on_msg_clone = on_msg.clone();
        let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
            if let Some(txt) = e.data().as_string() {
                if let Ok(resp) = serde_json::from_str::<HostResponse>(&txt) {
                    on_msg_clone.call(resp);
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        Ok(Self { ws })
    }

    pub fn send(&self, req: ClientRequest) -> Result<(), JsValue> {
        let txt = serde_json::to_string(&req).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.ws.send_with_str(&txt)
    }
}
