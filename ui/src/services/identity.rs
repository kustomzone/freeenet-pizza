//! User Identity Service
//!
//! Manages the user's signing key for cryptographic operations.
//! In a full implementation, this would communicate with a Freenet delegate.

use ed25519_dalek::{SigningKey, VerifyingKey};
use pizza_common::UserIdKey;
use std::cell::RefCell;

thread_local! {
    static IDENTITY: RefCell<Option<UserIdentity>> = RefCell::new(None);
}

/// User identity containing signing credentials
#[derive(Clone)]
pub struct UserIdentity {
    signing_key: SigningKey,
}

impl UserIdentity {
    /// Create a new random identity
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        let signing_key = SigningKey::generate(&mut rng);
        UserIdentity { signing_key }
    }

    /// Get the user's verifying (public) key
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Get the user ID key for use in state
    pub fn user_id(&self) -> UserIdKey {
        UserIdKey::from(&self.verifying_key())
    }

    /// Get the signing key for operations
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    /// Load or create identity from local storage
    pub fn load_or_create() -> Self {
        // Try to load from localStorage
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(key_hex)) = storage.get_item("pizza_identity") {
                    if let Ok(key_bytes) = hex_decode(&key_hex) {
                        if key_bytes.len() == 32 {
                            let mut bytes = [0u8; 32];
                            bytes.copy_from_slice(&key_bytes);
                            let signing_key = SigningKey::from_bytes(&bytes);
                            return UserIdentity { signing_key };
                        }
                    }
                }
            }
        }

        // Generate new identity and save
        let identity = Self::generate();
        identity.save();
        identity
    }

    /// Save identity to local storage
    pub fn save(&self) {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                let key_hex = hex_encode(self.signing_key.as_bytes());
                let _ = storage.set_item("pizza_identity", &key_hex);
            }
        }
    }
}

/// Initialize the global identity
pub fn init_identity() -> UserIdentity {
    let identity = UserIdentity::load_or_create();
    IDENTITY.with(|id| {
        *id.borrow_mut() = Some(identity.clone());
    });
    identity
}

/// Get the current identity
pub fn get_identity() -> Option<UserIdentity> {
    IDENTITY.with(|id| id.borrow().clone())
}

// Simple hex encoding/decoding
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}
