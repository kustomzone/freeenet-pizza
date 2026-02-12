//! Pizza Order State
//!
//! State structure for a collaborative pizza ordering contract.
//! Supports commutative merging for eventual consistency across peers.

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use freenet_scaffold::ComposableState;

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
// Operation types - for modifying state
// ============================================================================

/// Operations for modifying order configuration (creator only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderOperation {
    /// Create/initialize the order
    Create(CreateOrderOp),
    /// Update the order name
    UpdateName(UpdateNameOp),
}

/// Create a new order (creator only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderOp {
    pub name: String,
    pub created_at: DateTime<Utc>,
}

/// Update order name (creator only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNameOp {
    pub name: String,
    pub version: u64,
}

/// Operations for modifying order items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ItemOperation {
    /// Add a new item
    Add(AddItemOp),
    /// Edit an existing item
    Edit(EditItemOp),
    /// Delete an item
    Delete(DeleteItemOp),
    /// Update paid status (creator only)
    UpdatePaid(UpdatePaidOp),
}

/// Add an item to the order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddItemOp {
    pub display_name: String,
    pub order: String,
    pub price_cents: u64,
}

/// Edit own item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditItemOp {
    pub display_name: Option<String>,
    pub order: Option<String>,
    pub price_cents: Option<u64>,
    pub version: u64,
}

/// Delete own item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteItemOp {
    pub version: u64,
}

/// Update paid status for a user (creator only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePaidOp {
    pub user: UserIdKey,
    pub paid: bool,
    pub version: u64,
}

/// Top-level operation for modifying pizza order state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PizzaOrderOperation {
    /// Configuration operations (creator only)
    Config(OrderOperation),
    /// Item operations (any user for their own items, creator for paid status)
    Items(ItemOperation),
}

/// Context for applying operations (includes signing key for authorization)
pub struct OperationContext<'a> {
    /// The signing key of the user performing the operation
    pub signing_key: &'a SigningKey,
}

impl<'a> OperationContext<'a> {
    pub fn new(signing_key: &'a SigningKey) -> Self {
        Self { signing_key }
    }

    pub fn author(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    pub fn user_id(&self) -> UserIdKey {
        UserIdKey(self.signing_key.verifying_key().to_bytes())
    }
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

impl PizzaOrderState {
    pub fn apply_operation(
        &mut self,
        params: &PizzaOrderParameters,
        op: &PizzaOrderOperation,
        ctx: &OperationContext,
    ) -> Result<(), String> {
        match op {
            PizzaOrderOperation::Config(config_op) => {
                self.config.apply_operation(params, config_op, ctx)
            }
            PizzaOrderOperation::Items(items_op) => {
                self.items.apply_operation(params, items_op, ctx)
            }
        }
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

    pub fn apply_operation(
        &mut self,
        params: &PizzaOrderParameters,
        op: &OrderOperation,
        ctx: &OperationContext,
    ) -> Result<(), String> {
        // Only creator can modify configuration
        if ctx.author() != params.creator {
            return Err("Only creator can modify order configuration".to_string());
        }

        match op {
            OrderOperation::Create(create_op) => {
                // Can only create if not already created
                if self.version > 0 {
                    return Err("Order already created".to_string());
                }

                self.name = create_op.name.clone();
                self.created_at = Some(create_op.created_at);
                self.version = 1;

                let message = self.signing_message();
                self.signature = Some(ctx.signing_key.sign(&message));
                Ok(())
            }
            OrderOperation::UpdateName(update_op) => {
                // Version must be greater than current
                if update_op.version <= self.version {
                    return Err(format!(
                        "Update version {} must be greater than current version {}",
                        update_op.version, self.version
                    ));
                }

                self.name = update_op.name.clone();
                self.version = update_op.version;

                let message = self.signing_message();
                self.signature = Some(ctx.signing_key.sign(&message));
                Ok(())
            }
        }
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

impl OrderItems {
    pub fn apply_operation(
        &mut self,
        params: &PizzaOrderParameters,
        op: &ItemOperation,
        ctx: &OperationContext,
    ) -> Result<(), String> {
        let user_id = ctx.user_id();

        match op {
            ItemOperation::Add(add_op) => {
                // User can only have one item
                if self.items.contains_key(&user_id) {
                    return Err("User already has an item in this order".to_string());
                }

                let mut item = OrderItem {
                    display_name: add_op.display_name.clone(),
                    order: add_op.order.clone(),
                    price_cents: add_op.price_cents,
                    paid: false,
                    version: 1,
                    signature: Signature::from_bytes(&[0u8; 64]), // placeholder
                    signed_by: user_id.clone(),
                };

                let message = item.signing_message(&user_id);
                item.signature = ctx.signing_key.sign(&message);

                self.items.insert(user_id, item);
                Ok(())
            }
            ItemOperation::Edit(edit_op) => {
                let item = self
                    .items
                    .get(&user_id)
                    .ok_or_else(|| "User has no item to edit".to_string())?;

                // Version must be greater than current
                if edit_op.version <= item.version {
                    return Err(format!(
                        "Edit version {} must be greater than current version {}",
                        edit_op.version, item.version
                    ));
                }

                let mut new_item = item.clone();

                if let Some(ref name) = edit_op.display_name {
                    new_item.display_name = name.clone();
                }
                if let Some(ref order) = edit_op.order {
                    new_item.order = order.clone();
                }
                if let Some(price) = edit_op.price_cents {
                    new_item.price_cents = price;
                }

                new_item.version = edit_op.version;
                new_item.signed_by = user_id.clone();

                let message = new_item.signing_message(&user_id);
                new_item.signature = ctx.signing_key.sign(&message);

                self.items.insert(user_id, new_item);
                Ok(())
            }
            ItemOperation::Delete(delete_op) => {
                let item = self
                    .items
                    .get(&user_id)
                    .ok_or_else(|| "User has no item to delete".to_string())?;

                // Version must match for delete
                if delete_op.version != item.version {
                    return Err(format!(
                        "Delete version {} must match current version {}",
                        delete_op.version, item.version
                    ));
                }

                self.items.remove(&user_id);
                Ok(())
            }
            ItemOperation::UpdatePaid(paid_op) => {
                // Only creator can update paid status
                if ctx.author() != params.creator {
                    return Err("Only creator can update paid status".to_string());
                }

                let item = self
                    .items
                    .get(&paid_op.user)
                    .ok_or_else(|| "User has no item to update".to_string())?;

                // Version must be greater than current
                if paid_op.version <= item.version {
                    return Err(format!(
                        "Update version {} must be greater than current version {}",
                        paid_op.version, item.version
                    ));
                }

                let mut new_item = item.clone();
                new_item.paid = paid_op.paid;
                new_item.version = paid_op.version;
                new_item.signed_by = ctx.user_id();

                let message = new_item.signing_message(&paid_op.user);
                new_item.signature = ctx.signing_key.sign(&message);

                self.items.insert(paid_op.user.clone(), new_item);
                Ok(())
            }
        }
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
