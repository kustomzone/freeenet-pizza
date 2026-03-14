use serde::{Deserialize, Serialize};

/// Unique identifier for a signing request (for request/response correlation)
pub type RequestId = u64;

/// Messages sent from the App to the Pizza Delegate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PizzaDelegateRequest {
    /// Store contract keys (list of contract key strings the user has interacted with)
    StoreContractKeys { keys: Vec<String> },
    /// Get stored contract keys
    GetContractKeys,

    /// Store the user's signing key (32 bytes)
    StoreSigningKey { signing_key_bytes: [u8; 32] },
    /// Get the public key for the stored signing key
    GetPublicKey,
    /// Sign arbitrary data with the stored signing key
    Sign {
        request_id: RequestId,
        data: Vec<u8>,
    },
}

/// Responses sent from the Pizza Delegate to the App
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PizzaDelegateResponse {
    /// Response to StoreContractKeys
    StoreContractKeysResponse { result: Result<(), String> },
    /// Response to GetContractKeys
    GetContractKeysResponse { keys: Vec<String> },

    /// Response to StoreSigningKey
    StoreSigningKeyResponse { result: Result<(), String> },
    /// Response to GetPublicKey
    GetPublicKeyResponse {
        /// The public key bytes if the signing key exists
        public_key: Option<[u8; 32]>,
    },
    /// Response to Sign
    SignResponse {
        request_id: RequestId,
        /// The signature bytes (64 bytes for Ed25519)
        signature: Result<Vec<u8>, String>,
    },
}
