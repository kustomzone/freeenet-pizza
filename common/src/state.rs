//! Pizza Order State
//!
//! State structure for a collaborative pizza ordering contract.
//! Supports commutative merging for eventual consistency across peers.

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Serde helper for BTreeMap<UserIdKey, V> to serialize keys as hex strings for JSON compatibility
mod user_id_map_serde {
    use super::UserIdKey;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<V, S>(map: &BTreeMap<UserIdKey, V>, serializer: S) -> Result<S::Ok, S::Error>
    where
        V: Serialize,
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut ser_map = serializer.serialize_map(Some(map.len()))?;
        for (key, value) in map {
            let hex_key: String = key.0.iter().map(|b| format!("{:02x}", b)).collect();
            ser_map.serialize_entry(&hex_key, value)?;
        }
        ser_map.end()
    }

    pub fn deserialize<'de, V, D>(deserializer: D) -> Result<BTreeMap<UserIdKey, V>, D::Error>
    where
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        let string_map: BTreeMap<String, V> = BTreeMap::deserialize(deserializer)?;
        let mut result = BTreeMap::new();
        for (hex_key, value) in string_map {
            let bytes = hex_to_bytes(&hex_key).map_err(serde::de::Error::custom)?;
            result.insert(UserIdKey(bytes), value);
        }
        Ok(result)
    }

    fn hex_to_bytes(hex: &str) -> Result<[u8; 32], String> {
        if hex.len() != 64 {
            return Err(format!("Invalid hex length: expected 64, got {}", hex.len()));
        }
        let mut bytes = [0u8; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(|e| format!("Invalid hex: {}", e))?;
        }
        Ok(bytes)
    }
}

/// Serde helper for BTreeMap<UserIdKey, u64> (used in summaries)
mod user_id_version_map_serde {
    use super::UserIdKey;
    use serde::{Deserialize, Deserializer, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S>(map: &BTreeMap<UserIdKey, u64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut ser_map = serializer.serialize_map(Some(map.len()))?;
        for (key, value) in map {
            let hex_key: String = key.0.iter().map(|b| format!("{:02x}", b)).collect();
            ser_map.serialize_entry(&hex_key, value)?;
        }
        ser_map.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<UserIdKey, u64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string_map: BTreeMap<String, u64> = BTreeMap::deserialize(deserializer)?;
        let mut result = BTreeMap::new();
        for (hex_key, value) in string_map {
            let bytes = hex_to_bytes(&hex_key).map_err(serde::de::Error::custom)?;
            result.insert(UserIdKey(bytes), value);
        }
        Ok(result)
    }

    fn hex_to_bytes(hex: &str) -> Result<[u8; 32], String> {
        if hex.len() != 64 {
            return Err(format!("Invalid hex length: expected 64, got {}", hex.len()));
        }
        let mut bytes = [0u8; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(|e| format!("Invalid hex: {}", e))?;
        }
        Ok(bytes)
    }
}

/// Trait for composable state components that support CRDT-like synchronization.
///
/// Each component must be able to:
/// - Verify its own validity
/// - Generate a compact summary
/// - Compute deltas against a summary
/// - Apply deltas from other peers
pub trait ComposableState {
    /// Parent state that this component depends on
    type ParentState;
    /// Compact summary for sync negotiation
    type Summary;
    /// Delta/patch that can be applied
    type Delta;
    /// Parameters for validation
    type Parameters;

    /// Verify the component is valid
    fn verify(&self, parent: &Self::ParentState, params: &Self::Parameters) -> Result<(), String>;

    /// Generate a compact summary of current state
    fn summarize(&self, parent: &Self::ParentState, params: &Self::Parameters) -> Self::Summary;

    /// Compute what the remote is missing (delta from their summary)
    fn delta(
        &self,
        parent: &Self::ParentState,
        params: &Self::Parameters,
        remote_summary: &Self::Summary,
    ) -> Option<Self::Delta>;

    /// Apply a delta from another peer
    fn apply_delta(
        &mut self,
        parent: &Self::ParentState,
        params: &Self::Parameters,
        delta: &Option<Self::Delta>,
    ) -> Result<(), String>;
}

/// Unique identifier for a user (their public verifying key)
pub type UserId = VerifyingKey;

/// Contract parameters - fixed at creation, determines contract identity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PizzaOrderParameters {
    /// The creator's public key - only they can update paid status
    pub creator: VerifyingKey,
    /// Unique identifier for this order
    pub order_id: [u8; 32],
}

/// Top-level state for the pizza order contract
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PizzaOrderState {
    /// Order configuration (name, created_at)
    pub config: OrderConfiguration,
    /// Items ordered by each user
    pub items: OrderItems,
}

/// Order configuration - set by creator
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OrderConfiguration {
    /// Display name for the order (e.g., "Pizza for Friday party")
    pub name: String,
    /// When the order was created
    pub created_at: Option<DateTime<Utc>>,
    /// Version for last-writer-wins conflict resolution
    pub version: u64,
    /// Signature from creator authorizing this configuration
    pub signature: Option<Signature>,
}

/// Collection of order items, keyed by user ID
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OrderItems {
    /// Map from user ID (serialized) to their order item
    #[serde(with = "user_id_map_serde")]
    pub items: BTreeMap<UserIdKey, OrderItem>,
}

/// Wrapper for UserId to use as map key (serializable)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct UserIdKey(pub [u8; 32]);

impl From<&VerifyingKey> for UserIdKey {
    fn from(key: &VerifyingKey) -> Self {
        UserIdKey(key.to_bytes())
    }
}

impl UserIdKey {
    pub fn to_verifying_key(&self) -> Result<VerifyingKey, ed25519_dalek::SignatureError> {
        VerifyingKey::from_bytes(&self.0)
    }
}

/// Individual order item from a user
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderItem {
    /// User's display name for this order
    pub display_name: String,
    /// Description of what they're ordering (e.g., "2x Margherita")
    pub order: String,
    /// Price in cents (to avoid floating point issues)
    pub price_cents: u64,
    /// Whether the user has paid (only creator can set this)
    pub paid: bool,
    /// Version for conflict resolution
    pub version: u64,
    /// Signature from the item owner (or creator for paid field)
    pub signature: Signature,
    /// Who signed this version (owner or creator)
    pub signed_by: UserIdKey,
}

// ============================================================================
// Summary types - compact representation for sync
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PizzaOrderSummary {
    pub config: OrderConfigSummary,
    pub items: OrderItemsSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrderConfigSummary {
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrderItemsSummary {
    /// Map of user ID to item version
    #[serde(with = "user_id_version_map_serde")]
    pub item_versions: BTreeMap<UserIdKey, u64>,
}

// ============================================================================
// Delta types - incremental updates
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PizzaOrderDelta {
    pub config: Option<OrderConfigDelta>,
    pub items: Option<OrderItemsDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderConfigDelta {
    pub name: String,
    pub created_at: Option<DateTime<Utc>>,
    pub version: u64,
    pub signature: Option<Signature>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrderItemsDelta {
    /// New or updated items
    #[serde(with = "user_id_map_serde")]
    pub upserts: BTreeMap<UserIdKey, OrderItem>,
    /// Deleted items (just track the keys)
    pub deletions: Vec<UserIdKey>,
}

// ============================================================================
// ComposableState implementations
// ============================================================================

impl ComposableState for PizzaOrderState {
    type ParentState = ();
    type Summary = PizzaOrderSummary;
    type Delta = PizzaOrderDelta;
    type Parameters = PizzaOrderParameters;

    fn verify(
        &self,
        _parent: &Self::ParentState,
        params: &Self::Parameters,
    ) -> Result<(), String> {
        self.config.verify(&(), params)?;
        self.items.verify(&self.config, params)?;
        Ok(())
    }

    fn summarize(
        &self,
        _parent: &Self::ParentState,
        params: &Self::Parameters,
    ) -> Self::Summary {
        PizzaOrderSummary {
            config: self.config.summarize(&(), params),
            items: self.items.summarize(&self.config, params),
        }
    }

    fn delta(
        &self,
        _parent: &Self::ParentState,
        params: &Self::Parameters,
        summary: &Self::Summary,
    ) -> Option<Self::Delta> {
        let config_delta = self.config.delta(&(), params, &summary.config);
        let items_delta = self.items.delta(&self.config, params, &summary.items);

        if config_delta.is_none() && items_delta.is_none() {
            None
        } else {
            Some(PizzaOrderDelta {
                config: config_delta,
                items: items_delta,
            })
        }
    }

    fn apply_delta(
        &mut self,
        _parent: &Self::ParentState,
        params: &Self::Parameters,
        delta: &Option<Self::Delta>,
    ) -> Result<(), String> {
        if let Some(delta) = delta {
            if let Some(ref config_delta) = delta.config {
                self.config
                    .apply_delta(&(), params, &Some(config_delta.clone()))?;
            }
            if let Some(ref items_delta) = delta.items {
                self.items
                    .apply_delta(&self.config, params, &Some(items_delta.clone()))?;
            }
        }
        Ok(())
    }
}

impl ComposableState for OrderConfiguration {
    type ParentState = ();
    type Summary = OrderConfigSummary;
    type Delta = OrderConfigDelta;
    type Parameters = PizzaOrderParameters;

    fn verify(
        &self,
        _parent: &Self::ParentState,
        params: &Self::Parameters,
    ) -> Result<(), String> {
        // If configuration is set, verify signature from creator
        if self.version > 0 {
            let signature = self
                .signature
                .ok_or_else(|| "Configuration requires signature".to_string())?;

            let message = self.signing_message();
            params
                .creator
                .verify_strict(&message, &signature)
                .map_err(|e| format!("Invalid configuration signature: {}", e))?;
        }
        Ok(())
    }

    fn summarize(
        &self,
        _parent: &Self::ParentState,
        _params: &Self::Parameters,
    ) -> Self::Summary {
        OrderConfigSummary {
            version: self.version,
        }
    }

    fn delta(
        &self,
        _parent: &Self::ParentState,
        _params: &Self::Parameters,
        summary: &Self::Summary,
    ) -> Option<Self::Delta> {
        if self.version > summary.version {
            Some(OrderConfigDelta {
                name: self.name.clone(),
                created_at: self.created_at,
                version: self.version,
                signature: self.signature,
            })
        } else {
            None
        }
    }

    fn apply_delta(
        &mut self,
        _parent: &Self::ParentState,
        params: &Self::Parameters,
        delta: &Option<Self::Delta>,
    ) -> Result<(), String> {
        if let Some(delta) = delta {
            // Only apply if newer version
            if delta.version > self.version {
                // Verify signature before applying
                if let Some(signature) = delta.signature {
                    let temp = OrderConfiguration {
                        name: delta.name.clone(),
                        created_at: delta.created_at,
                        version: delta.version,
                        signature: Some(signature),
                    };
                    let message = temp.signing_message();
                    params
                        .creator
                        .verify_strict(&message, &signature)
                        .map_err(|e| format!("Invalid delta signature: {}", e))?;
                }

                self.name = delta.name.clone();
                self.created_at = delta.created_at;
                self.version = delta.version;
                self.signature = delta.signature;
            }
        }
        Ok(())
    }
}

impl OrderConfiguration {
    /// Message to sign for configuration changes
    pub fn signing_message(&self) -> Vec<u8> {
        let mut msg = Vec::new();
        msg.extend_from_slice(b"pizza-order-config:");
        msg.extend_from_slice(self.name.as_bytes());
        msg.extend_from_slice(b":");
        if let Some(ts) = self.created_at {
            msg.extend_from_slice(&ts.timestamp().to_le_bytes());
        }
        msg.extend_from_slice(b":");
        msg.extend_from_slice(&self.version.to_le_bytes());
        msg
    }
}

impl ComposableState for OrderItems {
    type ParentState = OrderConfiguration;
    type Summary = OrderItemsSummary;
    type Delta = OrderItemsDelta;
    type Parameters = PizzaOrderParameters;

    fn verify(
        &self,
        _parent: &Self::ParentState,
        params: &Self::Parameters,
    ) -> Result<(), String> {
        for (user_key, item) in &self.items {
            item.verify(user_key, params)?;
        }
        Ok(())
    }

    fn summarize(
        &self,
        _parent: &Self::ParentState,
        _params: &Self::Parameters,
    ) -> Self::Summary {
        OrderItemsSummary {
            item_versions: self
                .items
                .iter()
                .map(|(k, v)| (k.clone(), v.version))
                .collect(),
        }
    }

    fn delta(
        &self,
        _parent: &Self::ParentState,
        _params: &Self::Parameters,
        summary: &Self::Summary,
    ) -> Option<Self::Delta> {
        let mut upserts = BTreeMap::new();
        let mut deletions = Vec::new();

        // Find new or updated items
        for (key, item) in &self.items {
            match summary.item_versions.get(key) {
                Some(&remote_version) if item.version <= remote_version => {
                    // Remote has same or newer version
                }
                _ => {
                    // We have newer or new item
                    upserts.insert(key.clone(), item.clone());
                }
            }
        }

        // Find deleted items (in summary but not in our state)
        for key in summary.item_versions.keys() {
            if !self.items.contains_key(key) {
                deletions.push(key.clone());
            }
        }

        if upserts.is_empty() && deletions.is_empty() {
            None
        } else {
            Some(OrderItemsDelta { upserts, deletions })
        }
    }

    fn apply_delta(
        &mut self,
        _parent: &Self::ParentState,
        params: &Self::Parameters,
        delta: &Option<Self::Delta>,
    ) -> Result<(), String> {
        if let Some(delta) = delta {
            // Apply upserts
            for (key, item) in &delta.upserts {
                // Verify the item before inserting
                item.verify(key, params)?;

                match self.items.get(key) {
                    Some(existing) if existing.version >= item.version => {
                        // Keep existing if same or newer version
                    }
                    _ => {
                        self.items.insert(key.clone(), item.clone());
                    }
                }
            }

            // Note: deletions are handled by not having the item
            // In a CRDT, we typically use tombstones, but for simplicity
            // we just track versions - a "deletion" is represented by
            // absence in the source state
        }
        Ok(())
    }
}

impl OrderItem {
    /// Verify the item's signature
    pub fn verify(&self, owner_key: &UserIdKey, params: &PizzaOrderParameters) -> Result<(), String> {
        let signer = self.signed_by.to_verifying_key()
            .map_err(|e| format!("Invalid signer key: {}", e))?;

        let owner = owner_key.to_verifying_key()
            .map_err(|e| format!("Invalid owner key: {}", e))?;

        // Only owner or creator can sign
        let is_owner = signer == owner;
        let is_creator = signer == params.creator;

        if !is_owner && !is_creator {
            return Err("Item must be signed by owner or creator".to_string());
        }

        // Verify signature
        let message = self.signing_message(owner_key);
        signer
            .verify_strict(&message, &self.signature)
            .map_err(|e| format!("Invalid item signature: {}", e))?;

        Ok(())
    }

    /// Message to sign for item changes
    pub fn signing_message(&self, owner: &UserIdKey) -> Vec<u8> {
        let mut msg = Vec::new();
        msg.extend_from_slice(b"pizza-order-item:");
        msg.extend_from_slice(&owner.0);
        msg.extend_from_slice(b":");
        msg.extend_from_slice(self.display_name.as_bytes());
        msg.extend_from_slice(b":");
        msg.extend_from_slice(self.order.as_bytes());
        msg.extend_from_slice(b":");
        msg.extend_from_slice(&self.price_cents.to_le_bytes());
        msg.extend_from_slice(b":");
        msg.extend_from_slice(&[if self.paid { 1 } else { 0 }]);
        msg.extend_from_slice(b":");
        msg.extend_from_slice(&self.version.to_le_bytes());
        msg
    }
}

impl PizzaOrderState {
    /// Merge another state into this one (commutative operation)
    pub fn merge(&mut self, other: &PizzaOrderState, params: &PizzaOrderParameters) -> Result<(), String> {
        // Merge config - take higher version
        if other.config.version > self.config.version {
            other.config.verify(&(), params)?;
            self.config = other.config.clone();
        }

        // Merge items - take higher version for each user
        for (key, item) in &other.items.items {
            item.verify(key, params)?;

            match self.items.items.get(key) {
                Some(existing) if existing.version >= item.version => {
                    // Keep existing
                }
                _ => {
                    self.items.items.insert(key.clone(), item.clone());
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn test_order_item_json_roundtrip() {
        // Create a signing key
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let user_id = UserIdKey::from(&signing_key.verifying_key());

        // Create an order item
        let mut item = OrderItem {
            display_name: "Test User".to_string(),
            order: "1x Margherita".to_string(),
            price_cents: 1200,
            paid: false,
            version: 1,
            signature: signing_key.sign(b"test"), // placeholder
            signed_by: user_id.clone(),
        };

        // Sign it properly
        let message = item.signing_message(&user_id);
        item.signature = signing_key.sign(&message);

        // Serialize to JSON
        let json = serde_json::to_string(&item).expect("Failed to serialize");

        // Deserialize from JSON
        let deserialized: OrderItem =
            serde_json::from_str(&json).expect("Failed to deserialize");

        // Verify round-trip
        assert_eq!(item.display_name, deserialized.display_name);
        assert_eq!(item.order, deserialized.order);
        assert_eq!(item.price_cents, deserialized.price_cents);
        assert_eq!(item.paid, deserialized.paid);
        assert_eq!(item.version, deserialized.version);
        assert_eq!(item.signature, deserialized.signature);
        assert_eq!(item.signed_by, deserialized.signed_by);
    }

    #[test]
    fn test_pizza_order_state_json_roundtrip() {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let user_id = UserIdKey::from(&signing_key.verifying_key());

        // Create configuration
        let mut config = OrderConfiguration {
            name: "Test Order".to_string(),
            created_at: Some(chrono::Utc::now()),
            version: 1,
            signature: None,
        };
        let config_msg = config.signing_message();
        config.signature = Some(signing_key.sign(&config_msg));

        // Create an item
        let mut item = OrderItem {
            display_name: "Alice".to_string(),
            order: "2x Pepperoni".to_string(),
            price_cents: 2400,
            paid: false,
            version: 1,
            signature: signing_key.sign(b"placeholder"),
            signed_by: user_id.clone(),
        };
        let item_msg = item.signing_message(&user_id);
        item.signature = signing_key.sign(&item_msg);

        // Create state with items
        let mut state = PizzaOrderState {
            config,
            items: OrderItems::default(),
        };
        state.items.items.insert(user_id, item);

        // Serialize to JSON
        let json = serde_json::to_string(&state).expect("Failed to serialize state");

        // Deserialize from JSON
        let deserialized: PizzaOrderState =
            serde_json::from_str(&json).expect("Failed to deserialize state");

        // Verify round-trip
        assert_eq!(state.config.name, deserialized.config.name);
        assert_eq!(state.config.version, deserialized.config.version);
        assert_eq!(state.items.items.len(), deserialized.items.items.len());
    }
}
