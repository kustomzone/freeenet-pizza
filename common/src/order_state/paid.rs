use std::collections::HashMap;
use crate::util::truncated_base64;
use crate::{FullOrderStateV1, UserId};
use ed25519_dalek::{Signature, SignatureError, Signer, SigningKey, Verifier, VerifyingKey};
use freenet_scaffold::util::{fast_hash, FastHash};
use freenet_scaffold::ComposableState;
use serde::{Deserialize, Serialize};
use std::fmt;
use crate::order_state::OrderParametersV1;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct AuthorizedPaidV1 {
    pub paid: Paid,
    pub signature: Signature,
}

impl ComposableState for AuthorizedPaidV1 {
    type ParentState = FullOrderStateV1;
    type Summary = u32;
    type Delta = AuthorizedPaidV1;
    type Parameters = OrderParametersV1;

    fn verify(
        &self,
        parent_state: &Self::ParentState,
        parameters: &Self::Parameters,
    ) -> Result<(), String> {
        self.verify_signature(&parameters.owner)
            .map_err(|e| format!("Invalid signature: {}", e))?;

        for user_id in self.paid.values.keys() {
            if !parent_state
                .items
                .items
                .iter()
                .any(|item| item.item.signed_by == *user_id)
            {
                return Err(format!(
                    "User {:?} has a paid entry but no corresponding item",
                    user_id
                ));
            }
        }

        Ok(())
    }

    fn summarize(
        &self,
        _parent_state: &Self::ParentState,
        _parameters: &Self::Parameters,
    ) -> Self::Summary {
        self.paid.paid_version
    }

    fn delta(
        &self,
        _parent_state: &Self::ParentState,
        _parameters: &Self::Parameters,
        old_version: &Self::Summary,
    ) -> Option<Self::Delta> {
        if self.paid.paid_version > *old_version {
            Some(self.clone())
        } else {
            None
        }
    }

    fn apply_delta(
        &mut self,
        parent_state: &Self::ParentState,
        parameters: &Self::Parameters,
        delta: &Option<Self::Delta>,
    ) -> Result<(), String> {
        if let Some(delta) = delta {
            // Verify the delta's signature
            delta
                .verify_signature(&parameters.owner)
                .map_err(|e| format!("Invalid signature: {}", e))?;

            // Check if the new version is greater than the current version
            if delta.paid.paid_version <= self.paid.paid_version
            {
                return Err(
                    "New configuration version must be greater than the current version"
                        .to_string(),
                );
            }

            // Verify if all the entries in HashMap have corrosponding entry in _parent_state.items
            for user_id in delta.paid.values.keys() {
                if !parent_state
                    .items
                    .items
                    .iter()
                    .any(|item| item.item.signed_by == *user_id)
                {
                    return Err(format!(
                        "User {:?} has a paid entry but no corresponding item",
                        user_id
                    ));
                }
            }

            // If all checks pass, apply the delta
            self.paid = delta.paid.clone();
            self.signature = delta.signature;
        }

        Ok(())
    }
}

impl AuthorizedPaidV1 {
    pub fn new(paid: Paid, owner_signing_key: &SigningKey) -> Self {
        let mut serialized_paid = Vec::new();
        ciborium::ser::into_writer(&paid, &mut serialized_paid)
            .expect("Serialization should not fail");
        let signature = owner_signing_key.sign(&serialized_paid);

        Self {
            paid,
            signature,
        }
    }

    /// Create an AuthorizedPaidV1 with a pre-computed signature.
    /// Use this when signing is done externally (e.g., via delegate).
    pub fn with_signature(paid: Paid, signature: Signature) -> Self {
        Self {
            paid,
            signature,
        }
    }

    pub fn verify_signature(
        &self,
        owner_verifying_key: &VerifyingKey,
    ) -> Result<(), SignatureError> {
        let mut serialized_paid = Vec::new();
        ciborium::ser::into_writer(&self.paid, &mut serialized_paid)
            .expect("Serialization should not fail");
        owner_verifying_key.verify(&serialized_paid, &self.signature)
    }

    pub fn id(&self) -> FastHash {
        fast_hash(&self.signature.to_bytes())
    }
}

impl Default for AuthorizedPaidV1 {
    fn default() -> Self {
        let default_order = Paid::default();
        let default_key = SigningKey::from_bytes(&[0; 32]);
        Self::new(default_order, &default_key)
    }
}

impl Default for Paid {
    fn default() -> Self {
        Paid {
            values: HashMap::new(),
            paid_version: 0,
        }
    }
}

impl fmt::Debug for AuthorizedPaidV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthorizedOrder")
            .field("order", &self.paid)
            .field(
                "signature",
                &format_args!("{}", truncated_base64(self.signature.to_bytes())),
            )
            .finish()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Paid {
    pub values: HashMap<UserId, bool>,
    pub paid_version: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order_state::items::{AuthorizedItemV1, ItemV1, ItemContentV1};
    use crate::order_state::OrderParametersV1;
    use chrono::Utc;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_paid_validation() {
        let mut rng = OsRng;
        let owner_signing_key = SigningKey::generate(&mut rng);
        let owner_verifying_key = owner_signing_key.verifying_key();
        
        let user_signing_key = SigningKey::generate(&mut rng);
        let user_id = user_signing_key.verifying_key();

        let parameters = OrderParametersV1 {
            owner: owner_verifying_key,
            created_at: Utc::now(),
        };

        let mut parent_state = FullOrderStateV1::default();
        
        // 1. Test: Paid entry with no corresponding item should fail
        let mut paid_values = HashMap::new();
        paid_values.insert(user_id, true);
        let paid = Paid {
            values: paid_values,
            paid_version: 1,
        };
        let auth_paid = AuthorizedPaidV1::new(paid, &owner_signing_key);

        let result = auth_paid.verify(&parent_state, &parameters);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("has a paid entry but no corresponding item"));

        // 2. Test: Paid entry WITH corresponding item should pass
        let item = ItemV1 {
            signed_by: user_id,
            owner_sign: false,
            version: 1,
            content: ItemContentV1::Item {
                display_name: "Test".to_string(),
                order: "Pizza".to_string(),
                price_cents: 1000,
            },
        };
        let auth_item = AuthorizedItemV1::new(item, &user_signing_key);
        parent_state.items.items.push(auth_item);

        let result = auth_paid.verify(&parent_state, &parameters);
        assert!(result.is_ok());
        
        // 3. Test: apply_delta should also fail if item is missing
        let empty_parent_state = FullOrderStateV1::default();
        let mut current_paid = AuthorizedPaidV1::default();
        let result = current_paid.apply_delta(&empty_parent_state, &parameters, &Some(auth_paid));
        assert!(result.is_err());
    }
}