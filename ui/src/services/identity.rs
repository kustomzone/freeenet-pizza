//! User Identity Service
//!
//! Manages the user's signing key for cryptographic operations.
//! In a full implementation, this would communicate with a Freenet delegate.

use ed25519_dalek::{SigningKey, VerifyingKey};
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


    /// Get the signing key for operations
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    /// Load or create identity. 
    /// In a full Freenet implementation, this would fetch from the delegate.
    pub fn load_or_create() -> Self {
        // Since we are refactoring to Freenet, we no longer use localstorage.
        // For now, we generate a new identity for the session.
        // A complete implementation would use the Freenet delegate to persist and retrieve the user's identity.
        Self::generate()
    }

    /// Save identity.
    pub fn save(&self) {
        // No-op for now as we removed localstorage.
        // Integration with delegate for persistence should be done via FreenetService.
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
