//! Operations for updating pizza order state
//!
//! All operations are signed by the user performing them.

use crate::state::{OrderItem, UserIdKey};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

/// All possible operations on the pizza order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PizzaOperation {
    /// Initialize the order (creator only)
    CreateOrder(CreateOrderOp),
    /// Update the order name (creator only)
    UpdateName(UpdateNameOp),
    /// Update paid status for a user (creator only)
    UpdatePaid(UpdatePaidOp),
    /// Add an item to the order (any user)
    AddItem(AddItemOp),
    /// Edit own item
    EditItem(EditItemOp),
    /// Delete own item
    DeleteItem(DeleteItemOp),
}

/// Signed operation wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedOperation {
    pub operation: PizzaOperation,
    pub author: VerifyingKey,
    pub signature: Signature,
}

impl SignedOperation {
    /// Create a signed operation
    pub fn new(operation: PizzaOperation, signing_key: &SigningKey) -> Self {
        let author = signing_key.verifying_key();
        let message = operation.signing_message();
        let signature = signing_key.sign(&message);

        SignedOperation {
            operation,
            author,
            signature,
        }
    }

    /// Verify the operation signature
    pub fn verify(&self) -> Result<(), String> {
        let message = self.operation.signing_message();
        self.author
            .verify_strict(&message, &self.signature)
            .map_err(|e| format!("Invalid operation signature: {}", e))
    }
}

impl PizzaOperation {
    /// Generate message bytes for signing
    pub fn signing_message(&self) -> Vec<u8> {
        let mut msg = Vec::new();
        match self {
            PizzaOperation::CreateOrder(op) => {
                msg.extend_from_slice(b"create-order:");
                msg.extend_from_slice(op.name.as_bytes());
                msg.extend_from_slice(b":");
                msg.extend_from_slice(&op.created_at.timestamp().to_le_bytes());
            }
            PizzaOperation::UpdateName(op) => {
                msg.extend_from_slice(b"update-name:");
                msg.extend_from_slice(op.name.as_bytes());
                msg.extend_from_slice(b":");
                msg.extend_from_slice(&op.version.to_le_bytes());
            }
            PizzaOperation::UpdatePaid(op) => {
                msg.extend_from_slice(b"update-paid:");
                msg.extend_from_slice(&op.user.0);
                msg.extend_from_slice(b":");
                msg.extend_from_slice(&[if op.paid { 1 } else { 0 }]);
                msg.extend_from_slice(b":");
                msg.extend_from_slice(&op.version.to_le_bytes());
            }
            PizzaOperation::AddItem(op) => {
                msg.extend_from_slice(b"add-item:");
                msg.extend_from_slice(op.display_name.as_bytes());
                msg.extend_from_slice(b":");
                msg.extend_from_slice(op.order.as_bytes());
                msg.extend_from_slice(b":");
                msg.extend_from_slice(&op.price_cents.to_le_bytes());
            }
            PizzaOperation::EditItem(op) => {
                msg.extend_from_slice(b"edit-item:");
                if let Some(ref name) = op.display_name {
                    msg.extend_from_slice(name.as_bytes());
                }
                msg.extend_from_slice(b":");
                if let Some(ref order) = op.order {
                    msg.extend_from_slice(order.as_bytes());
                }
                msg.extend_from_slice(b":");
                if let Some(price) = op.price_cents {
                    msg.extend_from_slice(&price.to_le_bytes());
                }
                msg.extend_from_slice(b":");
                msg.extend_from_slice(&op.version.to_le_bytes());
            }
            PizzaOperation::DeleteItem(op) => {
                msg.extend_from_slice(b"delete-item:");
                msg.extend_from_slice(&op.version.to_le_bytes());
            }
        }
        msg
    }
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

/// Update paid status for a user (creator only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePaidOp {
    pub user: UserIdKey,
    pub paid: bool,
    pub version: u64,
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

// Helper to create order items from operations
impl OrderItem {
    /// Create a new order item from an add operation
    pub fn from_add_op(
        op: &AddItemOp,
        owner: &UserIdKey,
        signing_key: &SigningKey,
    ) -> Self {
        let mut item = OrderItem {
            display_name: op.display_name.clone(),
            order: op.order.clone(),
            price_cents: op.price_cents,
            paid: false,
            version: 1,
            signature: Signature::from_bytes(&[0u8; 64]), // placeholder
            signed_by: UserIdKey(signing_key.verifying_key().to_bytes()),
        };

        let message = item.signing_message(owner);
        item.signature = signing_key.sign(&message);
        item
    }

    /// Apply an edit operation
    pub fn apply_edit(
        &self,
        op: &EditItemOp,
        owner: &UserIdKey,
        signing_key: &SigningKey,
    ) -> Self {
        let mut item = self.clone();

        if let Some(ref name) = op.display_name {
            item.display_name = name.clone();
        }
        if let Some(ref order) = op.order {
            item.order = order.clone();
        }
        if let Some(price) = op.price_cents {
            item.price_cents = price;
        }

        item.version = op.version;
        item.signed_by = UserIdKey(signing_key.verifying_key().to_bytes());

        let message = item.signing_message(owner);
        item.signature = signing_key.sign(&message);
        item
    }

    /// Update paid status (creator only)
    pub fn with_paid_status(
        &self,
        paid: bool,
        version: u64,
        owner: &UserIdKey,
        signing_key: &SigningKey,
    ) -> Self {
        let mut item = self.clone();
        item.paid = paid;
        item.version = version;
        item.signed_by = UserIdKey(signing_key.verifying_key().to_bytes());

        let message = item.signing_message(owner);
        item.signature = signing_key.sign(&message);
        item
    }
}
