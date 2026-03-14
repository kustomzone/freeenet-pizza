use super::*;

/// Helper function to create an app response
pub(crate) fn create_app_response<T: Serialize>(
    response: &T,
) -> Result<OutboundDelegateMsg, DelegateError> {
    // Serialize response
    let mut response_bytes = Vec::new();
    ciborium::ser::into_writer(response, &mut response_bytes)
        .map_err(|e| DelegateError::Deser(format!("Failed to serialize response: {e}")))?;

    logging::info(&format!(
        "Creating app response with {} bytes",
        response_bytes.len()
    ));

    // Create response message
    let app_msg = ApplicationMessage::new(response_bytes)
        .with_context(DelegateContext::default())
        .processed(true);

    Ok(OutboundDelegateMsg::ApplicationMessage(app_msg))
}
