use super::*;
use ed25519_dalek::{Signer, SigningKey};
use freenet_stdlib::prelude::DelegateCtx;
use pizza_common::order_delegate::RequestId;

/// Handle an application message using the host function API for direct secret access.
pub(crate) fn handle_application_message(
    ctx: &mut DelegateCtx,
    app_msg: ApplicationMessage,
    origin: &Origin,
) -> Result<Vec<OutboundDelegateMsg>, DelegateError> {
    // Deserialize the request message
    let request: PizzaDelegateRequest = ciborium::from_reader(app_msg.payload.as_slice())
        .map_err(|e| DelegateError::Deser(format!("Failed to deserialize request: {e}")))?;

    match request {
        PizzaDelegateRequest::StoreContractKeys { keys } => {
            logging::info(&format!(
                "Delegate received StoreContractKeys with {} keys",
                keys.len()
            ));
            handle_store_contract_keys(ctx, origin, keys)
        }
        PizzaDelegateRequest::GetContractKeys => {
            logging::info("Delegate received GetContractKeys");
            handle_get_contract_keys(ctx, origin)
        }
        PizzaDelegateRequest::StoreSigningKey { signing_key_bytes } => {
            logging::info("Delegate received StoreSigningKey");
            handle_store_signing_key(ctx, origin, signing_key_bytes)
        }
        PizzaDelegateRequest::GetPublicKey => {
            logging::info("Delegate received GetPublicKey");
            handle_get_public_key(ctx, origin)
        }
        PizzaDelegateRequest::Sign { request_id, data } => {
            logging::info(&format!(
                "Delegate received Sign request_id={}, data_len={}",
                request_id,
                data.len()
            ));
            handle_sign(ctx, origin, request_id, data)
        }
    }
}

// ============================================================================
// Contract Keys Handlers
// ============================================================================

/// Create storage key for contract keys
fn contract_keys_storage_key(origin: &Origin) -> Vec<u8> {
    format!("{}{}", origin.to_b58(), CONTRACT_KEYS_SUFFIX).into_bytes()
}

/// Handle store contract keys request
fn handle_store_contract_keys(
    ctx: &mut DelegateCtx,
    origin: &Origin,
    keys: Vec<String>,
) -> Result<Vec<OutboundDelegateMsg>, DelegateError> {
    let storage_key = contract_keys_storage_key(origin);

    // Serialize the keys
    let mut value = Vec::new();
    ciborium::ser::into_writer(&keys, &mut value)
        .map_err(|e| DelegateError::Deser(format!("Failed to serialize contract keys: {e}")))?;

    // Store via host function
    #[cfg(target_family = "wasm")]
    if !ctx.set_secret(&storage_key, &value) {
        return Err(DelegateError::Other(
            "Failed to store contract keys via host function".into(),
        ));
    }
    #[cfg(not(target_family = "wasm"))]
    let _ = ctx.set_secret(&storage_key, &value);

    logging::info(&format!("Stored {} contract keys", keys.len()));

    let response = PizzaDelegateResponse::StoreContractKeysResponse { result: Ok(()) };
    Ok(vec![create_app_response(&response)?])
}

/// Handle get contract keys request
fn handle_get_contract_keys(
    ctx: &mut DelegateCtx,
    origin: &Origin,
) -> Result<Vec<OutboundDelegateMsg>, DelegateError> {
    let storage_key = contract_keys_storage_key(origin);

    let keys = ctx
        .get_secret(&storage_key)
        .and_then(|data| ciborium::from_reader::<Vec<String>, _>(data.as_slice()).ok())
        .unwrap_or_default();

    logging::info(&format!("Retrieved {} contract keys", keys.len()));

    let response = PizzaDelegateResponse::GetContractKeysResponse { keys };
    Ok(vec![create_app_response(&response)?])
}

// ============================================================================
// Signing Key Handlers
// ============================================================================

/// Create storage key for signing key
fn signing_key_storage_key(origin: &Origin) -> Vec<u8> {
    format!("{}{}", origin.to_b58(), SIGNING_KEY_SUFFIX).into_bytes()
}

/// Handle store signing key request
fn handle_store_signing_key(
    ctx: &mut DelegateCtx,
    origin: &Origin,
    signing_key_bytes: [u8; 32],
) -> Result<Vec<OutboundDelegateMsg>, DelegateError> {
    let storage_key = signing_key_storage_key(origin);

    // Store via host function
    #[cfg(target_family = "wasm")]
    if !ctx.set_secret(&storage_key, &signing_key_bytes) {
        return Err(DelegateError::Other(
            "Failed to store signing key via host function".into(),
        ));
    }
    #[cfg(not(target_family = "wasm"))]
    let _ = ctx.set_secret(&storage_key, &signing_key_bytes);

    logging::info("Stored signing key");

    let response = PizzaDelegateResponse::StoreSigningKeyResponse { result: Ok(()) };
    Ok(vec![create_app_response(&response)?])
}

/// Handle get public key request
fn handle_get_public_key(
    ctx: &mut DelegateCtx,
    origin: &Origin,
) -> Result<Vec<OutboundDelegateMsg>, DelegateError> {
    let storage_key = signing_key_storage_key(origin);

    let public_key = ctx.get_secret(&storage_key).and_then(|sk_bytes| {
        if sk_bytes.len() == 32 {
            let sk_array: [u8; 32] = sk_bytes.try_into().ok()?;
            let signing_key = SigningKey::from_bytes(&sk_array);
            Some(signing_key.verifying_key().to_bytes())
        } else {
            None
        }
    });

    logging::info(&format!(
        "Retrieved public key, present: {}",
        public_key.is_some()
    ));

    let response = PizzaDelegateResponse::GetPublicKeyResponse { public_key };
    Ok(vec![create_app_response(&response)?])
}

/// Handle sign request
fn handle_sign(
    ctx: &mut DelegateCtx,
    origin: &Origin,
    request_id: RequestId,
    data: Vec<u8>,
) -> Result<Vec<OutboundDelegateMsg>, DelegateError> {
    let storage_key = signing_key_storage_key(origin);

    let signature: Result<Vec<u8>, String> = match ctx.get_secret(&storage_key) {
        Some(sk_bytes) => {
            if sk_bytes.len() == 32 {
                let sk_array: [u8; 32] = sk_bytes.try_into().expect("length verified");
                let signing_key = SigningKey::from_bytes(&sk_array);
                let sig = signing_key.sign(&data);
                Ok(sig.to_bytes().to_vec())
            } else {
                Err(format!(
                    "Invalid signing key length: expected 32, got {}",
                    sk_bytes.len()
                ))
            }
        }
        None => Err("Signing key not found".to_string()),
    };

    logging::info(&format!(
        "Sign request completed, success: {}",
        signature.is_ok()
    ));

    let response = PizzaDelegateResponse::SignResponse {
        request_id,
        signature,
    };
    Ok(vec![create_app_response(&response)?])
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use freenet_stdlib::prelude::{ContractInstanceId, DelegateCtx, MessageOrigin};

    /// Helper function to create empty parameters for testing
    fn create_test_parameters() -> Parameters<'static> {
        Parameters::from(vec![])
    }

    /// Helper function to create an application message
    fn create_app_message(request: PizzaDelegateRequest) -> ApplicationMessage {
        let mut payload = Vec::new();
        ciborium::ser::into_writer(&request, &mut payload)
            .map_err(|e| panic!("Failed to serialize request: {e}"))
            .unwrap();
        ApplicationMessage::new(payload)
    }

    /// Create a test origin for use in tests
    fn get_test_origin() -> Option<MessageOrigin> {
        Some(MessageOrigin::WebApp(ContractInstanceId::new([42u8; 32])))
    }

    /// Helper function to extract response from outbound messages
    fn extract_response(messages: Vec<OutboundDelegateMsg>) -> Option<PizzaDelegateResponse> {
        for msg in messages {
            if let OutboundDelegateMsg::ApplicationMessage(app_msg) = msg {
                return ciborium::from_reader(app_msg.payload.as_slice())
                    .map_err(|e| panic!("Failed to deserialize response: {e}"))
                    .ok();
            }
        }
        None
    }

    #[test]
    fn test_store_contract_keys() {
        let keys = vec!["key1".to_string(), "key2".to_string()];
        let request = PizzaDelegateRequest::StoreContractKeys { keys: keys.clone() };
        let app_msg = create_app_message(request);
        let inbound_msg = InboundDelegateMsg::ApplicationMessage(app_msg);

        let result = crate::PizzaDelegate::process(
            &mut DelegateCtx::default(),
            create_test_parameters(),
            get_test_origin(),
            inbound_msg,
        )
        .unwrap();

        assert_eq!(result.len(), 1);

        let response = extract_response(result).unwrap();
        match response {
            PizzaDelegateResponse::StoreContractKeysResponse { result } => {
                assert!(result.is_ok());
            }
            _ => panic!("Expected StoreContractKeysResponse, got {:?}", response),
        }
    }

    #[test]
    fn test_get_contract_keys_empty() {
        let request = PizzaDelegateRequest::GetContractKeys;
        let app_msg = create_app_message(request);
        let inbound_msg = InboundDelegateMsg::ApplicationMessage(app_msg);

        let result = crate::PizzaDelegate::process(
            &mut DelegateCtx::default(),
            create_test_parameters(),
            get_test_origin(),
            inbound_msg,
        )
        .unwrap();

        assert_eq!(result.len(), 1);

        let response = extract_response(result).unwrap();
        match response {
            PizzaDelegateResponse::GetContractKeysResponse { keys } => {
                // Empty since we haven't stored anything (non-WASM test env)
                assert!(keys.is_empty());
            }
            _ => panic!("Expected GetContractKeysResponse, got {:?}", response),
        }
    }

    #[test]
    fn test_store_signing_key() {
        let signing_key_bytes: [u8; 32] = [8u8; 32];
        let request = PizzaDelegateRequest::StoreSigningKey { signing_key_bytes };
        let app_msg = create_app_message(request);
        let inbound_msg = InboundDelegateMsg::ApplicationMessage(app_msg);

        let result = crate::PizzaDelegate::process(
            &mut DelegateCtx::default(),
            create_test_parameters(),
            get_test_origin(),
            inbound_msg,
        )
        .unwrap();

        assert_eq!(result.len(), 1);

        let response = extract_response(result).unwrap();
        match response {
            PizzaDelegateResponse::StoreSigningKeyResponse { result } => {
                assert!(result.is_ok());
            }
            _ => panic!("Expected StoreSigningKeyResponse, got {:?}", response),
        }
    }

    #[test]
    fn test_get_public_key_not_found() {
        let request = PizzaDelegateRequest::GetPublicKey;
        let app_msg = create_app_message(request);
        let inbound_msg = InboundDelegateMsg::ApplicationMessage(app_msg);

        let result = crate::PizzaDelegate::process(
            &mut DelegateCtx::default(),
            create_test_parameters(),
            get_test_origin(),
            inbound_msg,
        )
        .unwrap();

        assert_eq!(result.len(), 1);

        let response = extract_response(result).unwrap();
        match response {
            PizzaDelegateResponse::GetPublicKeyResponse { public_key } => {
                // None since no key stored (non-WASM test env)
                assert!(public_key.is_none());
            }
            _ => panic!("Expected GetPublicKeyResponse, got {:?}", response),
        }
    }

    #[test]
    fn test_sign_without_key() {
        let request = PizzaDelegateRequest::Sign {
            request_id: 123,
            data: b"test data".to_vec(),
        };
        let app_msg = create_app_message(request);
        let inbound_msg = InboundDelegateMsg::ApplicationMessage(app_msg);

        let result = crate::PizzaDelegate::process(
            &mut DelegateCtx::default(),
            create_test_parameters(),
            get_test_origin(),
            inbound_msg,
        )
        .unwrap();

        assert_eq!(result.len(), 1);

        let response = extract_response(result).unwrap();
        match response {
            PizzaDelegateResponse::SignResponse {
                request_id,
                signature,
            } => {
                assert_eq!(request_id, 123);
                assert!(signature.is_err());
                assert!(signature.unwrap_err().contains("not found"));
            }
            _ => panic!("Expected SignResponse, got {:?}", response),
        }
    }

    #[test]
    fn test_error_on_missing_origin() {
        let request = PizzaDelegateRequest::GetContractKeys;
        let app_msg = create_app_message(request);
        let inbound_msg = InboundDelegateMsg::ApplicationMessage(app_msg);

        let result = crate::PizzaDelegate::process(
            &mut DelegateCtx::default(),
            create_test_parameters(),
            None,
            inbound_msg,
        );

        assert!(result.is_err());
        if let Err(DelegateError::Other(msg)) = result {
            assert!(msg.contains("missing message origin"));
        } else {
            panic!("Expected DelegateError::Other, got {:?}", result);
        }
    }

    #[test]
    fn test_error_on_processed_message() {
        let request = PizzaDelegateRequest::GetContractKeys;
        let mut payload = Vec::new();
        ciborium::ser::into_writer(&request, &mut payload).unwrap();

        let app_msg = ApplicationMessage::new(payload).processed(true);
        let inbound_msg = InboundDelegateMsg::ApplicationMessage(app_msg);

        let result = crate::PizzaDelegate::process(
            &mut DelegateCtx::default(),
            create_test_parameters(),
            get_test_origin(),
            inbound_msg,
        );

        assert!(result.is_err());
        if let Err(DelegateError::Other(msg)) = result {
            assert!(msg.contains("already processed"));
        } else {
            panic!("Expected DelegateError::Other, got {:?}", result);
        }
    }
}
