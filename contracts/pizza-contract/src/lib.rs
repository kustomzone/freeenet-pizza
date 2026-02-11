//! Pizza Order Contract
//!
//! A Freenet contract for collaborative pizza ordering.
//! Supports commutative state merging for eventual consistency.

use freenet_stdlib::prelude::*;
use pizza_common::{
    ComposableState, PizzaOrderDelta, PizzaOrderParameters, PizzaOrderState, PizzaOrderSummary,
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
        let params: PizzaOrderParameters = ciborium::from_reader(parameters.as_ref())
            .map_err(|e| ContractError::Deser(format!("Invalid parameters: {}", e)))?;

        let pizza_state: PizzaOrderState = ciborium::from_reader(state.as_ref())
            .map_err(|e| ContractError::Deser(format!("Invalid state: {}", e)))?;

        // Verify state using ComposableState
        pizza_state
            .verify(&(), &params)
            .map_err(|_| ContractError::InvalidState)?;

        Ok(ValidateResult::Valid)
    }

    /// Update state with new data (must be commutative)
    fn update_state(
        parameters: Parameters<'static>,
        state: State<'static>,
        data: Vec<UpdateData<'static>>,
    ) -> Result<UpdateModification<'static>, ContractError> {
        let params: PizzaOrderParameters = ciborium::from_reader(parameters.as_ref())
            .map_err(|e| ContractError::Deser(format!("Invalid parameters: {}", e)))?;

        let mut pizza_state: PizzaOrderState = if state.as_ref().is_empty() {
            PizzaOrderState::default()
        } else {
            ciborium::from_reader(state.as_ref())
                .map_err(|e| ContractError::Deser(format!("Invalid state: {}", e)))?
        };

        for update in data {
            match update {
                UpdateData::State(new_state) => {
                    let other: PizzaOrderState = ciborium::from_reader(new_state.as_ref())
                        .map_err(|e| ContractError::Deser(format!("Invalid update state: {}", e)))?;

                    // Merge states (commutative operation)
                    pizza_state
                        .merge(&other, &params)
                        .map_err(|_| ContractError::InvalidState)?;
                }
                UpdateData::Delta(delta) => {
                    let delta: PizzaOrderDelta = ciborium::from_reader(delta.as_ref())
                        .map_err(|e| ContractError::Deser(format!("Invalid delta: {}", e)))?;

                    // Apply delta
                    pizza_state
                        .apply_delta(&(), &params, &Some(delta))
                        .map_err(|_| ContractError::InvalidState)?;
                }
                UpdateData::StateAndDelta { state: _new_state, delta } => {
                    // Prefer delta for efficiency, but verify against state
                    let delta: PizzaOrderDelta = ciborium::from_reader(delta.as_ref())
                        .map_err(|e| ContractError::Deser(format!("Invalid delta: {}", e)))?;

                    pizza_state
                        .apply_delta(&(), &params, &Some(delta))
                        .map_err(|_| ContractError::InvalidState)?;
                }
                UpdateData::RelatedState { .. }
                | UpdateData::RelatedDelta { .. }
                | UpdateData::RelatedStateAndDelta { .. } => {
                    // This contract doesn't use related contracts
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
        let params: PizzaOrderParameters = ciborium::from_reader(parameters.as_ref())
            .map_err(|e| ContractError::Deser(format!("Invalid parameters: {}", e)))?;

        let pizza_state: PizzaOrderState = if state.as_ref().is_empty() {
            PizzaOrderState::default()
        } else {
            ciborium::from_reader(state.as_ref())
                .map_err(|e| ContractError::Deser(format!("Invalid state: {}", e)))?
        };

        let summary = pizza_state.summarize(&(), &params);

        let mut summary_bytes = Vec::new();
        ciborium::into_writer(&summary, &mut summary_bytes)
            .map_err(|e| ContractError::Deser(format!("Failed to serialize summary: {}", e)))?;

        Ok(StateSummary::from(summary_bytes))
    }

    /// Generate a delta from a summary (what the requester is missing)
    fn get_state_delta(
        parameters: Parameters<'static>,
        state: State<'static>,
        summary: StateSummary<'static>,
    ) -> Result<StateDelta<'static>, ContractError> {
        let params: PizzaOrderParameters = ciborium::from_reader(parameters.as_ref())
            .map_err(|e| ContractError::Deser(format!("Invalid parameters: {}", e)))?;

        let pizza_state: PizzaOrderState = if state.as_ref().is_empty() {
            PizzaOrderState::default()
        } else {
            ciborium::from_reader(state.as_ref())
                .map_err(|e| ContractError::Deser(format!("Invalid state: {}", e)))?
        };

        let remote_summary: PizzaOrderSummary = ciborium::from_reader(summary.as_ref())
            .map_err(|e| ContractError::Deser(format!("Invalid summary: {}", e)))?;

        let delta = pizza_state.delta(&(), &params, &remote_summary);

        let mut delta_bytes = Vec::new();
        ciborium::into_writer(&delta, &mut delta_bytes)
            .map_err(|e| ContractError::Deser(format!("Failed to serialize delta: {}", e)))?;

        Ok(StateDelta::from(delta_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use ed25519_dalek::{Signer, SigningKey};
    use pizza_common::{
        AddItemOp, OrderConfiguration, OrderItem, OrderItems, UserIdKey,
    };
    use rand::rngs::OsRng;

    fn create_test_params() -> (PizzaOrderParameters, SigningKey) {
        let creator_key = SigningKey::generate(&mut OsRng);
        let params = PizzaOrderParameters {
            creator: creator_key.verifying_key(),
            order_id: [0u8; 32],
        };
        (params, creator_key)
    }

    fn create_test_state(_params: &PizzaOrderParameters, creator_key: &SigningKey) -> PizzaOrderState {
        let mut config = OrderConfiguration {
            name: "Test Order".to_string(),
            created_at: Some(Utc::now()),
            version: 1,
            signature: None,
        };

        // Sign the config
        let message = config.signing_message();
        config.signature = Some(creator_key.sign(&message));

        PizzaOrderState {
            config,
            items: OrderItems::default(),
        }
    }

    #[test]
    fn test_merge_is_commutative() {
        let (params, creator_key) = create_test_params();

        // Create two users
        let user_a_key = SigningKey::generate(&mut OsRng);
        let user_b_key = SigningKey::generate(&mut OsRng);

        let user_a_id = UserIdKey::from(&user_a_key.verifying_key());
        let user_b_id = UserIdKey::from(&user_b_key.verifying_key());

        // Base state
        let base = create_test_state(&params, &creator_key);

        // User A adds an item
        let mut state_a = base.clone();
        let item_a = OrderItem::from_add_op(
            &AddItemOp {
                display_name: "Alice".to_string(),
                order: "1x Margherita".to_string(),
                price_cents: 1200,
            },
            &user_a_id,
            &user_a_key,
        );
        state_a.items.items.insert(user_a_id.clone(), item_a);

        // User B adds an item
        let mut state_b = base.clone();
        let item_b = OrderItem::from_add_op(
            &AddItemOp {
                display_name: "Bob".to_string(),
                order: "2x Pepperoni".to_string(),
                price_cents: 2400,
            },
            &user_b_id,
            &user_b_key,
        );
        state_b.items.items.insert(user_b_id.clone(), item_b);

        // Merge in both orders
        let mut merge_ab = state_a.clone();
        merge_ab.merge(&state_b, &params).unwrap();

        let mut merge_ba = state_b.clone();
        merge_ba.merge(&state_a, &params).unwrap();

        // Both should have the same result
        assert_eq!(merge_ab.items.items.len(), 2);
        assert_eq!(merge_ba.items.items.len(), 2);
        assert!(merge_ab.items.items.contains_key(&user_a_id));
        assert!(merge_ab.items.items.contains_key(&user_b_id));
        assert!(merge_ba.items.items.contains_key(&user_a_id));
        assert!(merge_ba.items.items.contains_key(&user_b_id));
    }

    #[test]
    fn test_validate_state() {
        let (params, creator_key) = create_test_params();
        let state = create_test_state(&params, &creator_key);

        // Serialize
        let mut params_bytes = Vec::new();
        ciborium::into_writer(&params, &mut params_bytes).unwrap();

        let mut state_bytes = Vec::new();
        ciborium::into_writer(&state, &mut state_bytes).unwrap();

        // Validate
        let result = PizzaContract::validate_state(
            Parameters::from(params_bytes),
            State::from(state_bytes),
            RelatedContracts::new(),
        );

        assert!(matches!(result, Ok(ValidateResult::Valid)));
    }

    #[test]
    fn test_delta_round_trip() {
        let (params, creator_key) = create_test_params();

        // Initial state
        let state_a = create_test_state(&params, &creator_key);

        // State with added item
        let mut state_b = state_a.clone();
        let user_key = SigningKey::generate(&mut OsRng);
        let user_id = UserIdKey::from(&user_key.verifying_key());
        let item = OrderItem::from_add_op(
            &AddItemOp {
                display_name: "Test User".to_string(),
                order: "1x Veggie".to_string(),
                price_cents: 1400,
            },
            &user_id,
            &user_key,
        );
        state_b.items.items.insert(user_id.clone(), item);

        // Generate summary from state_a
        let summary_a = state_a.summarize(&(), &params);

        // Generate delta from state_b (what state_a is missing)
        let delta = state_b.delta(&(), &params, &summary_a);
        assert!(delta.is_some());

        // Apply delta to state_a
        let mut reconstructed = state_a.clone();
        reconstructed.apply_delta(&(), &params, &delta).unwrap();

        // Should now have the item
        assert!(reconstructed.items.items.contains_key(&user_id));
        assert_eq!(reconstructed.items.items.len(), 1);
    }
}
