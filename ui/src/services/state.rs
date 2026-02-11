//! Application State Service
//!
//! Manages pizza orders and synchronizes with Freenet contracts.
//! Currently uses local storage as a mock backend.

use crate::services::PizzaInvite;
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use pizza_common::{
    AddItemOp, OrderConfiguration, OrderItem, OrderItems, PizzaOrderParameters, PizzaOrderState,
    UserIdKey,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A pizza order with its contract info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PizzaOrder {
    /// Unique identifier (contract key in real implementation)
    pub id: String,
    /// Contract parameters
    pub params: PizzaOrderParametersSerde,
    /// Current state
    pub state: PizzaOrderState,
}

/// Serializable version of PizzaOrderParameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PizzaOrderParametersSerde {
    pub creator: [u8; 32],
    pub order_id: [u8; 32],
}

impl PizzaOrderParametersSerde {
    pub fn to_params(&self) -> Result<PizzaOrderParameters, String> {
        Ok(PizzaOrderParameters {
            creator: VerifyingKey::from_bytes(&self.creator)
                .map_err(|e| format!("Invalid creator key: {}", e))?,
            order_id: self.order_id,
        })
    }
}

impl From<&PizzaOrderParameters> for PizzaOrderParametersSerde {
    fn from(params: &PizzaOrderParameters) -> Self {
        PizzaOrderParametersSerde {
            creator: params.creator.to_bytes(),
            order_id: params.order_id,
        }
    }
}

/// Application state stored in local storage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub orders: BTreeMap<String, PizzaOrder>,
}

impl AppState {
    /// Load state from local storage
    pub fn load() -> Self {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(json)) = storage.get_item("pizza_orders") {
                    if let Ok(state) = serde_json::from_str(&json) {
                        return state;
                    }
                }
            }
        }
        AppState::default()
    }

    /// Save state to local storage
    pub fn save(&self) {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(json) = serde_json::to_string(self) {
                    let _ = storage.set_item("pizza_orders", &json);
                }
            }
        }
    }

    /// Create a new pizza order
    pub fn create_order(&mut self, name: String, creator_key: &SigningKey) -> String {
        let order_id: [u8; 32] = rand::random();
        let id = hex_encode(&order_id[..8]);

        let params = PizzaOrderParameters {
            creator: creator_key.verifying_key(),
            order_id,
        };

        let mut config = OrderConfiguration {
            name,
            created_at: Some(Utc::now()),
            version: 1,
            signature: None,
        };

        // Sign the configuration
        let message = config.signing_message();
        config.signature = Some(creator_key.sign(&message));

        let state = PizzaOrderState {
            config,
            items: OrderItems::default(),
        };

        let order = PizzaOrder {
            id: id.clone(),
            params: PizzaOrderParametersSerde::from(&params),
            state,
        };

        self.orders.insert(id.clone(), order);
        self.save();
        id
    }

    /// Create an order from an invite (joining an existing order)
    pub fn create_order_from_invite(&mut self, invite: &PizzaInvite, _user_key: &SigningKey) {
        // In a real Freenet implementation, this would:
        // 1. Subscribe to the contract using the order_id
        // 2. Fetch the current state from the network
        // For now, we create a placeholder order

        // Use the invite's order_id directly
        let order_id = invite.order_id.clone();

        // Create a placeholder - in production this would be fetched from network
        let params = PizzaOrderParametersSerde {
            creator: [0u8; 32], // Unknown creator - would be fetched from contract
            order_id: [0u8; 32],
        };

        let config = OrderConfiguration {
            name: invite.order_name.clone(),
            created_at: Some(Utc::now()),
            version: 1,
            signature: None, // Would be fetched from contract
        };

        let state = PizzaOrderState {
            config,
            items: OrderItems::default(),
        };

        let order = PizzaOrder {
            id: order_id.clone(),
            params,
            state,
        };

        self.orders.insert(order_id, order);
        self.save();
    }

    /// Add an item to an order
    pub fn add_item(
        &mut self,
        order_id: &str,
        display_name: String,
        order_text: String,
        price_cents: u64,
        user_key: &SigningKey,
    ) -> Result<(), String> {
        let order = self
            .orders
            .get_mut(order_id)
            .ok_or_else(|| "Order not found".to_string())?;

        let user_id = UserIdKey::from(&user_key.verifying_key());

        let item = OrderItem::from_add_op(
            &AddItemOp {
                display_name,
                order: order_text,
                price_cents,
            },
            &user_id,
            user_key,
        );

        order.state.items.items.insert(user_id, item);
        self.save();
        Ok(())
    }

    /// Update an item in an order
    pub fn update_item(
        &mut self,
        order_id: &str,
        display_name: Option<String>,
        order_text: Option<String>,
        price_cents: Option<u64>,
        user_key: &SigningKey,
    ) -> Result<(), String> {
        let order = self
            .orders
            .get_mut(order_id)
            .ok_or_else(|| "Order not found".to_string())?;

        let user_id = UserIdKey::from(&user_key.verifying_key());

        let existing = order
            .state
            .items
            .items
            .get(&user_id)
            .ok_or_else(|| "Item not found".to_string())?
            .clone();

        let edit_op = pizza_common::EditItemOp {
            display_name,
            order: order_text,
            price_cents,
            version: existing.version + 1,
        };

        let updated = existing.apply_edit(&edit_op, &user_id, user_key);
        order.state.items.items.insert(user_id, updated);
        self.save();
        Ok(())
    }

    /// Delete an item from an order
    pub fn delete_item(&mut self, order_id: &str, user_key: &SigningKey) -> Result<(), String> {
        let order = self
            .orders
            .get_mut(order_id)
            .ok_or_else(|| "Order not found".to_string())?;

        let user_id = UserIdKey::from(&user_key.verifying_key());
        order.state.items.items.remove(&user_id);
        self.save();
        Ok(())
    }

    /// Update paid status (creator only)
    pub fn update_paid(
        &mut self,
        order_id: &str,
        target_user: &UserIdKey,
        paid: bool,
        creator_key: &SigningKey,
    ) -> Result<(), String> {
        let order = self
            .orders
            .get_mut(order_id)
            .ok_or_else(|| "Order not found".to_string())?;

        // Verify caller is creator
        let params = order.params.to_params()?;
        if creator_key.verifying_key() != params.creator {
            return Err("Only creator can update paid status".to_string());
        }

        let existing = order
            .state
            .items
            .items
            .get(target_user)
            .ok_or_else(|| "Item not found".to_string())?
            .clone();

        let updated =
            existing.with_paid_status(paid, existing.version + 1, target_user, creator_key);
        order.state.items.items.insert(target_user.clone(), updated);
        self.save();
        Ok(())
    }

    /// Update order name (creator only)
    pub fn update_order_name(
        &mut self,
        order_id: &str,
        name: String,
        creator_key: &SigningKey,
    ) -> Result<(), String> {
        let order = self
            .orders
            .get_mut(order_id)
            .ok_or_else(|| "Order not found".to_string())?;

        // Verify caller is creator
        let params = order.params.to_params()?;
        if creator_key.verifying_key() != params.creator {
            return Err("Only creator can update order name".to_string());
        }

        order.state.config.name = name;
        order.state.config.version += 1;

        let message = order.state.config.signing_message();
        order.state.config.signature = Some(creator_key.sign(&message));

        self.save();
        Ok(())
    }

    /// Delete an order
    pub fn delete_order(&mut self, order_id: &str) {
        self.orders.remove(order_id);
        self.save();
    }

    /// Get an order by ID
    pub fn get_order(&self, order_id: &str) -> Option<&PizzaOrder> {
        self.orders.get(order_id)
    }

    /// Get all orders
    pub fn get_orders(&self) -> Vec<&PizzaOrder> {
        self.orders.values().collect()
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
