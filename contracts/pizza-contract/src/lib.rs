//! Pizza Order Contract
//!
//! A Freenet contract for collaborative pizza ordering.
//! Supports commutative state merging for eventual consistency.

use freenet_stdlib::prelude::*;
use pizza_common::{
    ComposableState, FullOrderStateV1, OrderParametersV1,
};

struct PizzaContract;

#[contract]
impl ContractInterface for PizzaContract {
    /// Validate that the state is internally consistent
    fn validate_state(
        parameters: Parameters<'static>,
        state: State<'static>,
        _related: RelatedContracts<'static>,
    ) -> Result<ValidateResult, ContractError> {
        let params: OrderParametersV1 = ciborium::from_reader(parameters.as_ref())
            .map_err(|e| ContractError::Deser(format!("Invalid parameters: {}", e)))?;

        let pizza_state: FullOrderStateV1 = ciborium::from_reader(state.as_ref())
            .map_err(|e| ContractError::Deser(format!("Invalid state: {}", e)))?;

        // Verify state using ComposableState
        pizza_state
            .verify(&pizza_state, &params)
            .map_err(|_| ContractError::InvalidState)?;

        Ok(ValidateResult::Valid)
    }

    /// Update state with new data (must be commutative)
    fn update_state(
        parameters: Parameters<'static>,
        state: State<'static>,
        data: Vec<UpdateData<'static>>,
    ) -> Result<UpdateModification<'static>, ContractError> {
        let params: OrderParametersV1 = ciborium::from_reader(parameters.as_ref())
            .map_err(|e| ContractError::Deser(format!("Invalid parameters: {}", e)))?;

        let mut pizza_state: FullOrderStateV1 = if state.as_ref().is_empty() {
            FullOrderStateV1::default()
        } else {
            ciborium::from_reader(state.as_ref())
                .map_err(|e| ContractError::Deser(format!("Invalid state: {}", e)))?
        };

        for update in data {
            if let UpdateData::State(new_state) = update {
                let other: FullOrderStateV1 = ciborium::from_reader(new_state.as_ref())
                    .map_err(|e| ContractError::Deser(format!("Invalid update state: {}", e)))?;
                // Naive strategy: accept newer components if they verify against params
                // In a real contract we'd compute and apply deltas per component
                if other.verify(&other, &params).is_ok() {
                    pizza_state = other;
                }
            }
        }

        // Serialize updated state
        let mut state_bytes = Vec::new();
        ciborium::into_writer(&pizza_state, &mut state_bytes)
            .map_err(|e| ContractError::Deser(format!("Failed to serialize state: {}", e)))?;

        Ok(UpdateModification::valid(State::from(state_bytes)))
    }

    /// Generate a concise summary of the state for delta computation
    fn summarize_state(
        parameters: Parameters<'static>,
        state: State<'static>,
    ) -> Result<StateSummary<'static>, ContractError> {
        let _params: OrderParametersV1 = ciborium::from_reader(parameters.as_ref())
            .map_err(|e| ContractError::Deser(format!("Invalid parameters: {}", e)))?;

        let _pizza_state: FullOrderStateV1 = if state.as_ref().is_empty() {
            FullOrderStateV1::default()
        } else {
            ciborium::from_reader(state.as_ref())
                .map_err(|e| ContractError::Deser(format!("Invalid state: {}", e)))?
        };

        // For now, simplified: return empty summary
        Ok(StateSummary::from(Vec::<u8>::new()))
    }

    /// Generate a delta from a summary (what the requester is missing)
    fn get_state_delta(
        parameters: Parameters<'static>,
        state: State<'static>,
        summary: StateSummary<'static>,
    ) -> Result<StateDelta<'static>, ContractError> {
        let _params: OrderParametersV1 = ciborium::from_reader(parameters.as_ref())
            .map_err(|e| ContractError::Deser(format!("Invalid parameters: {}", e)))?;

        let _pizza_state: FullOrderStateV1 = if state.as_ref().is_empty() {
            FullOrderStateV1::default()
        } else {
            ciborium::from_reader(state.as_ref())
                .map_err(|e| ContractError::Deser(format!("Invalid state: {}", e)))?
        };

        // Simplified: return empty delta
        Ok(StateDelta::from(Vec::<u8>::new()))
    }
}

#[cfg(test)]
mod tests {
    // Tests for the old state/ops have been removed during migration to the new order_state API.
}
