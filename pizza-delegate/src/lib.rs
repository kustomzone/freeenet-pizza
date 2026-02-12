//! Pizza Delegate
//!
//! Handles user identity and signing operations for the pizza ordering app.
//! Runs locally on the user's device within the Freenet kernel.

use ed25519_dalek::{SigningKey, VerifyingKey};
use freenet_stdlib::prelude::*;
use serde::{Deserialize, Serialize};

/// Request types for the delegate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DelegateRequest {
    /// Get the user's public key
    GetPublicKey,
}

/// Response types from the delegate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DelegateResponse {
    /// The user's public key
    PublicKey(VerifyingKey),
    /// Error occurred
    Error(String),
}

/// The pizza delegate implementation
struct PizzaDelegate;

#[delegate]
impl DelegateInterface for PizzaDelegate {
    fn process(
        params: Parameters<'static>,
        _attested: Option<&'static [u8]>,
        message: InboundDelegateMsg<'_>,
    ) -> Result<Vec<OutboundDelegateMsg>, DelegateError> {
        match message {
            InboundDelegateMsg::ApplicationMessage(app_msg) => {
                // Deserialize the request
                let request: DelegateRequest = ciborium::from_reader(app_msg.payload.as_ref())
                    .map_err(|e| DelegateError::Deser(format!("Invalid request: {}", e)))?;

                // Get or generate the signing key from secret storage
                let signing_key = get_or_create_signing_key()?;

                let response = match request {
                    DelegateRequest::GetPublicKey => {
                        DelegateResponse::PublicKey(signing_key.verifying_key())
                    }

                };

                // Serialize response
                let mut response_bytes = Vec::new();
                ciborium::into_writer(&response, &mut response_bytes)
                    .map_err(|e| DelegateError::Deser(format!("Failed to serialize response: {}", e)))?;

                Ok(vec![OutboundDelegateMsg::ApplicationMessage(
                    ApplicationMessage::new(app_msg.app, response_bytes)
                        .processed(app_msg.processed),
                )])
            }

            InboundDelegateMsg::GetSecretRequest(req) => {
                // Handle secret storage requests
                Ok(vec![OutboundDelegateMsg::GetSecretResponse(
                    GetSecretResponse {
                        key: req.key,
                        value: None, // Let the kernel handle storage
                    },
                )])
            }

            _ => Ok(vec![]),
        }
    }
}

/// Get or create the signing key from secret storage
fn get_or_create_signing_key() -> Result<SigningKey, DelegateError> {
    // In a real implementation, this would use the delegate's secret storage
    // For now, we generate a deterministic key based on some seed
    // TODO: Implement proper secret storage integration

    // This is a placeholder - in production, the key would be stored securely
    let seed: [u8; 32] = [
        0x42, 0x69, 0x7a, 0x7a, 0x61, 0x2d, 0x6b, 0x65, 0x79, 0x2d, 0x73, 0x65, 0x65, 0x64, 0x2d,
        0x76, 0x31, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ];

    Ok(SigningKey::from_bytes(&seed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_public_key() {
        let signing_key = get_or_create_signing_key().unwrap();
        let verifying_key = signing_key.verifying_key();

        // Should be consistent
        let signing_key2 = get_or_create_signing_key().unwrap();
        assert_eq!(signing_key.verifying_key(), signing_key2.verifying_key());
        assert_eq!(verifying_key, signing_key2.verifying_key());
    }
}
