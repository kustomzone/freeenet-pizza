use crate::util::truncated_base64;
use crate::FullOrderStateV1;
use ed25519_dalek::{Signature, SignatureError, Signer, SigningKey, Verifier, VerifyingKey};
use freenet_scaffold::util::{fast_hash, FastHash};
use freenet_scaffold::ComposableState;
use serde::{Deserialize, Serialize};
use std::fmt;
use crate::order_state::OrderParametersV1;

pub const MAX_TEXT_LEN: usize = 120;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct AuthorizedOrderV1 {
    pub order: Order,
    pub signature: Signature,
}

impl ComposableState for AuthorizedOrderV1 {
    type ParentState = FullOrderStateV1;
    type Summary = u32;
    type Delta = AuthorizedOrderV1;
    type Parameters = OrderParametersV1;

    fn verify(
        &self,
        _parent_state: &Self::ParentState,
        parameters: &Self::Parameters,
    ) -> Result<(), String> {
        self.verify_signature(&parameters.owner)
            .map_err(|e| format!("Invalid signature: {}", e))
    }

    fn summarize(
        &self,
        _parent_state: &Self::ParentState,
        _parameters: &Self::Parameters,
    ) -> Self::Summary {
        self.order.order_version
    }

    fn delta(
        &self,
        _parent_state: &Self::ParentState,
        _parameters: &Self::Parameters,
        old_version: &Self::Summary,
    ) -> Option<Self::Delta> {
        if self.order.order_version > *old_version {
            Some(self.clone())
        } else {
            None
        }
    }

    fn apply_delta(
        &mut self,
        _parent_state: &Self::ParentState,
        parameters: &Self::Parameters,
        delta: &Option<Self::Delta>,
    ) -> Result<(), String> {
        if let Some(delta) = delta {
            // Verify the delta's signature
            delta
                .verify_signature(&parameters.owner)
                .map_err(|e| format!("Invalid signature: {}", e))?;

            // Check if the new version is greater than the current version
            if delta.order.order_version <= self.order.order_version
            {
                return Err(
                    "New configuration version must be greater than the current version"
                        .to_string(),
                );
            }

            // Validate display metadata declared lengths
            if delta.order.name.len() >= MAX_TEXT_LEN {
                return Err(format!(
                    "Order name declared length {} exceeds {}",
                    delta.order.name.len(),
                    MAX_TEXT_LEN
                ));
            }

            // If all checks pass, apply the delta
            self.order = delta.order.clone();
            self.signature = delta.signature;
        }

        Ok(())
    }
}

impl AuthorizedOrderV1 {
    pub fn new(order: Order, owner_signing_key: &SigningKey) -> Self {
        let mut serialized_order = Vec::new();
        ciborium::ser::into_writer(&order, &mut serialized_order)
            .expect("Serialization should not fail");
        let signature = owner_signing_key.sign(&serialized_order);

        Self {
            order,
            signature,
        }
    }

    /// Create an AuthorizedOrderV1 with a pre-computed signature.
    /// Use this when signing is done externally (e.g., via delegate).
    pub fn with_signature(order: Order, signature: Signature) -> Self {
        Self {
            order,
            signature,
        }
    }

    pub fn verify_signature(
        &self,
        owner_verifying_key: &VerifyingKey,
    ) -> Result<(), SignatureError> {
        let mut serialized_order = Vec::new();
        ciborium::ser::into_writer(&self.order, &mut serialized_order)
            .expect("Serialization should not fail");
        owner_verifying_key.verify(&serialized_order, &self.signature)
    }

    pub fn id(&self) -> FastHash {
        fast_hash(&self.signature.to_bytes())
    }
}

impl Default for AuthorizedOrderV1 {
    fn default() -> Self {
        let default_order = Order::default();
        let default_key = SigningKey::from_bytes(&[0; 32]);
        Self::new(default_order, &default_key)
    }
}

impl Default for Order {
    fn default() -> Self {
        Order {
            name: "".parse().unwrap(),
            order_version: 0,
        }
    }
}

impl fmt::Debug for AuthorizedOrderV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthorizedOrder")
            .field("order", &self.order)
            .field(
                "signature",
                &format_args!("{}", truncated_base64(self.signature.to_bytes())),
            )
            .finish()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Order {
    pub name: String,
    pub order_version: u32,
}
/*
#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn test_verify() {
        let owner_signing_key = SigningKey::generate(&mut OsRng);
        let owner_verifying_key = VerifyingKey::from(&owner_signing_key);
        let order = Order::default();
        let authorized_order =
            AuthorizedOrderV1::new(order.clone(), &owner_signing_key);

        assert!(authorized_order
            .verify_signature(&owner_verifying_key)
            .is_ok());

        let parent_state = FullOrderStateV1 {
            order: authorized_order.clone(),
            ..FullOrderStateV1::default()
        };
        let parameters = OrderParametersV1 {
            owner: owner_verifying_key,
        };

        assert!(authorized_order
            .verify(&parent_state, &parameters)
            .is_ok());
    }

    #[test]
    fn test_verify_fail() {
        let owner_signing_key = SigningKey::generate(&mut OsRng);
        let order = Order::default();
        let authorized_order =
            AuthorizedOrderV1::new(order.clone(), &owner_signing_key);

        let wrong_owner_signing_key = SigningKey::generate(&mut OsRng);
        let wrong_owner_verifying_key = VerifyingKey::from(&wrong_owner_signing_key);

        assert!(authorized_order
            .verify_signature(&wrong_owner_verifying_key)
            .is_err());

        let parent_state = FullOrderStateV1 {
            order: authorized_order.clone(),
            ..FullOrderStateV1::default()
        };
        let parameters = OrderParametersV1 {
            owner: wrong_owner_verifying_key,
        };

        assert!(authorized_order
            .verify(&parent_state, &parameters)
            .is_err());
    }

    #[test]
    fn test_summarize() {
        let owner_signing_key = SigningKey::generate(&mut OsRng);
        let owner_verifying_key = VerifyingKey::from(&owner_signing_key);
        let configuration = Order::default();
        let authorized_order =
            AuthorizedOrderV1::new(configuration.clone(), &owner_signing_key);

        let parent_state = FullOrderStateV1 {
            configuration: authorized_order.clone(),
            ..Default::default()
        };
        let parameters = OrderParametersV1 {
            owner: owner_verifying_key,
        };

        assert_eq!(
            authorized_order.summarize(&parent_state, &parameters),
            configuration.configuration_version
        );
    }

    #[test]
    fn test_delta_new_version() {
        let owner_signing_key = SigningKey::generate(&mut OsRng);
        let owner_verifying_key = VerifyingKey::from(&owner_signing_key);
        let configuration = Order::default();
        let authorized_order =
            AuthorizedOrderV1::new(configuration.clone(), &owner_signing_key);

        let parent_state = FullOrderStateV1 {
            configuration: authorized_order.clone(),
            ..Default::default()
        };
        let parameters = OrderParametersV1 {
            owner: owner_verifying_key,
        };

        let new_configuration = Configuration {
            configuration_version: 2,
            ..configuration.clone()
        };
        let new_authorized_order =
            AuthorizedOrderV1::new(new_configuration.clone(), &owner_signing_key);

        assert_eq!(
            new_authorized_order.delta(&parent_state, &parameters, &1),
            Some(new_authorized_order)
        );
    }

    #[test]
    fn test_delta_older_version() {
        let owner_signing_key = SigningKey::generate(&mut OsRng);
        let owner_verifying_key = VerifyingKey::from(&owner_signing_key);

        // Create an older configuration (version 1)
        let old_configuration = Configuration {
            configuration_version: 1,
            ..Order::default()
        };
        let old_authorized_order =
            AuthorizedOrderV1::new(old_configuration.clone(), &owner_signing_key);

        let parent_state = FullOrderStateV1 {
            configuration: old_authorized_order.clone(),
            ..Default::default()
        };
        let parameters = OrderParametersV1 {
            owner: owner_verifying_key,
        };

        // Test against a newer version (2)
        // The delta should return None since our configuration is older
        assert_eq!(
            old_authorized_order.delta(&parent_state, &parameters, &2),
            None
        );
    }

    #[test]
    fn test_apply_delta_should_apply() {
        let owner_signing_key = SigningKey::generate(&mut OsRng);
        let owner_verifying_key = VerifyingKey::from(&owner_signing_key);
        let configuration = Order::default();
        let mut authorized_order =
            AuthorizedOrderV1::new(configuration.clone(), &owner_signing_key);

        let parent_state = FullOrderStateV1 {
            configuration: authorized_order.clone(),
            ..Default::default()
        };
        let parameters = OrderParametersV1 {
            owner: owner_verifying_key,
        };

        let new_configuration = Configuration {
            configuration_version: 2,
            ..configuration.clone()
        };
        let new_authorized_order =
            AuthorizedOrderV1::new(new_configuration.clone(), &owner_signing_key);

        authorized_order
            .apply_delta(
                &parent_state,
                &parameters,
                &Some(new_authorized_order.clone()),
            )
            .unwrap();

        assert_eq!(authorized_order, new_authorized_order);
    }

    #[test]
    fn test_apply_delta_old_version() {
        let owner_signing_key = SigningKey::generate(&mut OsRng);
        let owner_verifying_key = VerifyingKey::from(&owner_signing_key);
        let configuration = Order::default();
        let mut authorized_order =
            AuthorizedOrderV1::new(configuration.clone(), &owner_signing_key);

        let orig_authorized_order = authorized_order.clone();

        let parent_state = FullOrderStateV1 {
            configuration: authorized_order.clone(),
            ..Default::default()
        };
        let parameters = OrderParametersV1 {
            owner: owner_verifying_key,
        };

        let new_configuration = Configuration {
            configuration_version: 0,
            ..configuration.clone()
        };
        let new_authorized_order =
            AuthorizedOrderV1::new(new_configuration.clone(), &owner_signing_key);

        let result = authorized_order.apply_delta(
            &parent_state,
            &parameters,
            &Some(new_authorized_order),
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "New configuration version must be greater than the current version"
        );
        assert_eq!(authorized_order, orig_authorized_order);
    }

    #[test]
    fn test_apply_delta_change_owner() {
        let owner_signing_key = SigningKey::generate(&mut OsRng);
        let owner_verifying_key = VerifyingKey::from(&owner_signing_key);
        let configuration = Configuration {
            owner_member_id: MemberId(FastHash(1)),
            ..Order::default()
        };
        let mut authorized_order =
            AuthorizedOrderV1::new(configuration.clone(), &owner_signing_key);

        let parent_state = FullOrderStateV1 {
            configuration: authorized_order.clone(),
            ..Default::default()
        };
        let parameters = OrderParametersV1 {
            owner: owner_verifying_key,
        };

        let mut new_configuration = configuration.clone();
        new_configuration.configuration_version += 1;
        new_configuration.owner_member_id = MemberId(FastHash(2));
        let new_authorized_order =
            AuthorizedOrderV1::new(new_configuration, &owner_signing_key);

        let result = authorized_order.apply_delta(
            &parent_state,
            &parameters,
            &Some(new_authorized_order),
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Cannot change the owner_member_id");
    }

    #[test]
    fn test_apply_delta_invalid_values() {
        let owner_signing_key = SigningKey::generate(&mut OsRng);
        let owner_verifying_key = VerifyingKey::from(&owner_signing_key);
        let configuration = Order::default();
        let mut authorized_order =
            AuthorizedOrderV1::new(configuration.clone(), &owner_signing_key);

        let parent_state = FullOrderStateV1 {
            configuration: authorized_order.clone(),
            ..Default::default()
        };
        let parameters = OrderParametersV1 {
            owner: owner_verifying_key,
        };

        let mut new_configuration = configuration.clone();
        new_configuration.configuration_version += 1;
        new_configuration.max_recent_messages = 0;
        let new_authorized_order =
            AuthorizedOrderV1::new(new_configuration, &owner_signing_key);

        let result = authorized_order.apply_delta(
            &parent_state,
            &parameters,
            &Some(new_authorized_order),
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid configuration values");
    }
}
*/