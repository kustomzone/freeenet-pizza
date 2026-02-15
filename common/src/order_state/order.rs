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

            if delta.order.currency.len() == 0 || delta.order.currency.len() > 3 {
                return Err(format!("Currency can be 1-3 chars, but is {} chars", delta.order.currency.len()))
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
            currency: "$".parse().unwrap(),
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
    pub currency: String,
    pub order_version: u32,
}
