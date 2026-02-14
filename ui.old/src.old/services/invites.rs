//! Invite Service
//!
//! Manages invite links for sharing pizza orders.
//! In a full Freenet implementation, invites would be cryptographically signed.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An invite to join a pizza order
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PizzaInvite {
    /// Unique invite ID
    pub id: String,
    /// Order ID to join
    pub order_id: String,
    /// Order name (for display)
    pub order_name: String,
    /// Who created the invite
    pub inviter_name: String,
    /// Timestamp when created
    pub created_at: i64,
}

/// Pending invites stored locally
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InviteStore {
    pub pending: BTreeMap<String, PizzaInvite>,
}

impl InviteStore {
    /// Load invites.
    /// In a real Freenet app, these would be managed by a delegate.
    pub fn load() -> Self {
        // Since we are refactoring away from localstorage, we'll start empty.
        // In a next step, we would fetch these from the Freenet node.
        InviteStore::default()
    }

    /// Save invites.
    pub fn save(&self) {
        // No-op for now as we removed localstorage.
    }

    /// Add a pending invite
    pub fn add_invite(&mut self, invite: PizzaInvite) {
        self.pending.insert(invite.id.clone(), invite);
        self.save();
    }

    /// Remove an invite (accepted or denied)
    pub fn remove_invite(&mut self, invite_id: &str) {
        self.pending.remove(invite_id);
        self.save();
    }

    /// Get all pending invites
    pub fn get_pending(&self) -> Vec<&PizzaInvite> {
        self.pending.values().collect()
    }
}

impl PizzaInvite {
    /// Create a new invite
    pub fn new(order_id: String, order_name: String, inviter_name: String) -> Self {
        let id = generate_invite_id();
        let created_at = chrono::Utc::now().timestamp();

        PizzaInvite {
            id,
            order_id,
            order_name,
            inviter_name,
            created_at,
        }
    }

    /// Encode invite as a URL-safe string
    pub fn to_link(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let encoded = base64_encode(json.as_bytes());

        // Get current origin for the link
        let origin = web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:8080".to_string());

        format!("{}/?invite={}", origin, encoded)
    }

    /// Decode invite from URL parameter
    pub fn from_link(encoded: &str) -> Option<Self> {
        let decoded = base64_decode(encoded)?;
        let json = String::from_utf8(decoded).ok()?;
        serde_json::from_str(&json).ok()
    }
}

/// Generate a random invite ID
fn generate_invite_id() -> String {
    let bytes: [u8; 8] = rand::random();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Base64 URL-safe encoding
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let mut result = String::new();
    let mut i = 0;

    while i < data.len() {
        let b0 = data[i] as usize;
        let b1 = if i + 1 < data.len() { data[i + 1] as usize } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] as usize } else { 0 };

        result.push(ALPHABET[b0 >> 2] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if i + 1 < data.len() {
            result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        }
        if i + 2 < data.len() {
            result.push(ALPHABET[b2 & 0x3f] as char);
        }

        i += 3;
    }

    result
}

/// Base64 URL-safe decoding
fn base64_decode(data: &str) -> Option<Vec<u8>> {
    const DECODE: [i8; 128] = [
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,62,-1,-1,
        52,53,54,55,56,57,58,59,60,61,-1,-1,-1,-1,-1,-1,
        -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,
        15,16,17,18,19,20,21,22,23,24,25,-1,-1,-1,-1,63,
        -1,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,
        41,42,43,44,45,46,47,48,49,50,51,-1,-1,-1,-1,-1,
    ];

    let mut result = Vec::new();
    let bytes: Vec<u8> = data.bytes().collect();
    let mut i = 0;

    while i < bytes.len() {
        let b0 = DECODE.get(bytes[i] as usize).copied().unwrap_or(-1);
        let b1 = bytes.get(i + 1).and_then(|&b| DECODE.get(b as usize).copied()).unwrap_or(-1);
        let b2 = bytes.get(i + 2).and_then(|&b| DECODE.get(b as usize).copied()).unwrap_or(0);
        let b3 = bytes.get(i + 3).and_then(|&b| DECODE.get(b as usize).copied()).unwrap_or(0);

        if b0 < 0 || b1 < 0 {
            return None;
        }

        result.push(((b0 << 2) | (b1 >> 4)) as u8);

        if i + 2 < bytes.len() && b2 >= 0 {
            result.push((((b1 & 0x0f) << 4) | (b2 >> 2)) as u8);
        }
        if i + 3 < bytes.len() && b3 >= 0 {
            result.push((((b2 & 0x03) << 6) | b3) as u8);
        }

        i += 4;
    }

    Some(result)
}

/// Check URL for invite parameter on page load
pub fn check_url_for_invite() -> Option<PizzaInvite> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;

    if search.starts_with("?invite=") || search.contains("&invite=") {
        let params: Vec<&str> = search.trim_start_matches('?').split('&').collect();
        for param in params {
            if let Some(encoded) = param.strip_prefix("invite=") {
                return PizzaInvite::from_link(encoded);
            }
        }
    }

    None
}

/// Clear invite from URL without reload
pub fn clear_invite_from_url() {
    if let Some(window) = web_sys::window() {
        if let Ok(history) = window.history() {
            let _ = history.replace_state_with_url(
                &wasm_bindgen::JsValue::NULL,
                "",
                Some("/"),
            );
        }
    }
}
