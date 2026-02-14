//! Application State Service
//!
//! Manages pizza orders and synchronizes with Freenet contracts.

use crate::services::{PizzaInvite, FreenetService, ClientRequest, ContractKey, WrappedState, WrappedDelta, ContractContainer};
use chrono::{DateTime, Utc};
use ed25519_dalek::{SigningKey, VerifyingKey};
use pizza_common::{
    FullOrderStateV1, OrderParametersV1, ComposableState,
};
use pizza_common::order_state::{AuthorizedOrderV1, Order, ItemsV1, AuthorizedItemV1, ItemV1, ItemContentV1, AuthorizedPaidV1, Paid};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A pizza order with its contract info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PizzaOrder {
    /// Unique identifier (contract key in real implementation)
    pub id: String,
    /// Contract parameters
    pub params: OrderParametersSerde,
    /// Current state
    pub state: FullOrderStateV1,
}

/// Serializable version of OrderParametersV1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderParametersSerde {
    pub owner: [u8; 32],
    pub created_at_rfc3339: String,
}

impl OrderParametersSerde {
    pub fn to_params(&self) -> Result<OrderParametersV1, String> {
        Ok(OrderParametersV1 {
            owner: VerifyingKey::from_bytes(&self.owner)
                .map_err(|e| format!("Invalid owner key: {}", e))?,
            created_at: DateTime::parse_from_rfc3339(&self.created_at_rfc3339)
                .map_err(|e| format!("Invalid created_at: {}", e))?
                .with_timezone(&Utc),
        })
    }
}

impl From<&OrderParametersV1> for OrderParametersSerde {
    fn from(params: &OrderParametersV1) -> Self {
        OrderParametersSerde {
            owner: params.owner.to_bytes(),
            created_at_rfc3339: params.created_at.to_rfc3339(),
        }
    }
}

/// Application state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub orders: BTreeMap<String, PizzaOrder>,
    #[serde(skip)]
    pub freenet: Option<FreenetService>,
}

impl AppState {
    /// Initialize AppState with FreenetService
    pub fn new() -> Self {
        AppState::default()
    }

    pub fn set_freenet(&mut self, freenet: FreenetService) {
        self.freenet = Some(freenet);
    }

    /// Update order state from Freenet
    pub fn update_from_freenet(&mut self, key: ContractKey, state_bytes: Vec<u8>) {
        if let Ok(state) = ciborium::de::from_reader::<FullOrderStateV1, &[u8]>(&state_bytes) {
            // We need to find the order by key.
            // For now, let's assume the key is the same as order id or we can derive it.
            // In a real app, we'd have a mapping from ContractKey to OrderId.
            if let Some(order) = self.orders.get_mut(&key.0) {
                order.state = state;
            } else {
                // New order discovered via subscription?
                // We don't have params yet, but we can create a placeholder
                let order = PizzaOrder {
                    id: key.0.clone(),
                    params: OrderParametersSerde {
                        owner: [0u8; 32],
                        created_at_rfc3339: Utc::now().to_rfc3339(),
                    },
                    state,
                };
                self.orders.insert(key.0, order);
            }
        }
    }

    /// Create a new pizza order
    pub fn create_order(&mut self, name: String, creator_key: &SigningKey) -> String {
        let random_id: [u8; 32] = rand::random();
        let id = hex_encode(&random_id[..8]);

        let params = OrderParametersV1 {
            owner: creator_key.verifying_key(),
            created_at: Utc::now(),
        };

        // Build initial state using new order_state API
        let order_msg = Order { name, order_version: 1 };
        let authorized_order = AuthorizedOrderV1::new(order_msg, creator_key);

        let state = FullOrderStateV1 {
            order: authorized_order,
            items: ItemsV1::default(),
            paid: AuthorizedPaidV1::new(Paid::default(), creator_key),
            ..Default::default()
        };

        let order = PizzaOrder {
            id: id.clone(),
            params: OrderParametersSerde::from(&params),
            state: state.clone(),
        };

        self.orders.insert(id.clone(), order);

        // Push to Freenet
        if let Some(freenet) = &self.freenet {
            let mut state_bytes = vec![];
            let _ = ciborium::ser::into_writer(&state, &mut state_bytes);
            let mut params_bytes = vec![];
            let _ = ciborium::ser::into_writer(&params, &mut params_bytes);

            let _ = freenet.send(ClientRequest::Put {
                contract: ContractContainer {
                    data: vec![], // In real Freenet, this would be the contract code WASM
                    parameters: params_bytes,
                },
                state: WrappedState(state_bytes),
            });
        }

        id
    }

    /// Create an order from an invite (joining an existing order)
    pub fn create_order_from_invite(&mut self, invite: &PizzaInvite, _user_key: &SigningKey) {
        let order_id = invite.order_id.clone();

        let params = OrderParametersSerde {
            owner: [0u8; 32], // Unknown owner - would be fetched from contract
            created_at_rfc3339: Utc::now().to_rfc3339(),
        };

        let mut state = FullOrderStateV1::default();
        // Set the visible name; in a real app we'd fetch a signed name from the network
        state.order.order.name = invite.order_name.clone();

        let order = PizzaOrder {
            id: order_id.clone(),
            params,
            state,
        };

        self.orders.insert(order_id.clone(), order);
        
        // Subscribe to Freenet
        if let Some(freenet) = &self.freenet {
            let _ = freenet.send(ClientRequest::Subscribe {
                key: ContractKey(order_id),
            });
        }
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
        let user_vk = user_key.verifying_key();
        
        let next_state = {
            let order = self
                .orders
                .get(order_id)
                .ok_or_else(|| "Order not found".to_string())?;

            // Determine next version for this user's item
            let next_version = order
                .state
                .items
                .items
                .iter()
                .find(|it| it.item.signed_by == user_vk)
                .map(|it| it.item.version + 1)
                .unwrap_or(1);

            let item = ItemV1 {
                signed_by: user_vk,
                owner_sign: false,
                version: next_version,
                content: ItemContentV1::Item {
                    display_name,
                    order: order_text,
                    price_cents,
                },
            };
            let auth_item = AuthorizedItemV1::new(item, user_key);

            let mut next_state = order.state.clone();

            // Replace existing entry for this user or push new
            if let Some(pos) = next_state
                .items
                .items
                .iter()
                .position(|it| it.item.signed_by == user_vk)
            {
                next_state.items.items[pos] = auth_item;
            } else {
                next_state.items.items.push(auth_item);
            }
            next_state
        };

        // Push to Freenet
        let old_state = self.orders.get(order_id).unwrap().state.clone();
        self.push_update(order_id, &old_state, &next_state)?;

        // Optimistically update local state
        if let Some(order) = self.orders.get_mut(order_id) {
            order.state = next_state;
        }

        Ok(())
    }

    /// Push an update to Freenet
    pub fn push_update(&self, order_id: &str, old_state: &FullOrderStateV1, new_state: &FullOrderStateV1) -> Result<(), String> {
        if let Some(freenet) = &self.freenet {
            let order = self.orders.get(order_id).ok_or("Order not found")?;
            let params = order.params.to_params()?;
            let summary = old_state.summarize(old_state, &params);
            if let Some(delta) = new_state.delta(old_state, &params, &summary) {
                let mut delta_bytes = vec![];
                ciborium::ser::into_writer(&delta, &mut delta_bytes).map_err(|e| e.to_string())?;
                
                freenet.send(ClientRequest::Update {
                    key: ContractKey(order_id.to_string()),
                    delta: WrappedDelta(delta_bytes),
                }).map_err(|e| format!("{:?}", e))?;
            }
        }
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
        let user_vk = user_key.verifying_key();

        let next_state = {
            let order = self
                .orders
                .get(order_id)
                .ok_or_else(|| "Order not found".to_string())?;

            let pos = order
                .state
                .items
                .items
                .iter()
                .position(|it| it.item.signed_by == user_vk)
                .ok_or_else(|| "Item not found".to_string())?;

            let existing = order.state.items.items[pos].clone();

            // Build updated item content
            let (mut disp, mut ord, mut price) = match existing.item.content {
                ItemContentV1::Item {
                    display_name,
                    order,
                    price_cents,
                } => (display_name, order, price_cents),
                ItemContentV1::Deleted { .. } => (String::new(), String::new(), 0),
            };
            if let Some(v) = display_name {
                disp = v;
            }
            if let Some(v) = order_text {
                ord = v;
            }
            if let Some(v) = price_cents {
                price = v;
            }

            let new_item = ItemV1 {
                signed_by: user_vk,
                owner_sign: false,
                version: existing.item.version + 1,
                content: ItemContentV1::Item {
                    display_name: disp,
                    order: ord,
                    price_cents: price,
                },
            };
            let auth_item = AuthorizedItemV1::new(new_item, user_key);

            let mut next_state = order.state.clone();
            next_state.items.items[pos] = auth_item;
            next_state
        };

        let old_state = self.orders.get(order_id).unwrap().state.clone();
        self.push_update(order_id, &old_state, &next_state)?;
        if let Some(order) = self.orders.get_mut(order_id) {
            order.state = next_state;
        }

        Ok(())
    }

    /// Delete an item from an order
    pub fn delete_item(&mut self, order_id: &str, user_key: &SigningKey) -> Result<(), String> {
        let user_vk = user_key.verifying_key();
        
        let (old_state, next_state) = {
            let order = self
                .orders
                .get(order_id)
                .ok_or_else(|| "Order not found".to_string())?;

            if let Some(pos) = order
                .state
                .items
                .items
                .iter()
                .position(|it| it.item.signed_by == user_vk)
            {
                let mut next_state = order.state.clone();
                next_state.items.items.remove(pos);
                (Some(order.state.clone()), Some(next_state))
            } else {
                (None, None)
            }
        };

        if let (Some(old), Some(next)) = (old_state, next_state) {
            self.push_update(order_id, &old, &next)?;
            if let Some(order) = self.orders.get_mut(order_id) {
                order.state = next;
            }
        }
        Ok(())
    }

    /// Update paid status (creator only)
    pub fn update_paid(
        &mut self,
        order_id: &str,
        target_user: &VerifyingKey,
        paid: bool,
        creator_key: &SigningKey,
    ) -> Result<(), String> {
        let (old_state, next_state) = {
            let order = self
                .orders
                .get(order_id)
                .ok_or_else(|| "Order not found".to_string())?;

            // Verify caller is owner
            let params = order.params.to_params()?;
            if creator_key.verifying_key() != params.owner {
                return Err("Only owner can update paid status".to_string());
            }

            // Start from current paid map
            let mut paid_map = order.state.paid.paid.values.clone();
            paid_map.insert(*target_user, paid);
            let new_paid = Paid {
                values: paid_map,
                paid_version: order.state.paid.paid.paid_version + 1,
            };

            let mut next_state = order.state.clone();
            next_state.paid = AuthorizedPaidV1::new(new_paid, creator_key);
            (order.state.clone(), next_state)
        };

        self.push_update(order_id, &old_state, &next_state)?;
        if let Some(order) = self.orders.get_mut(order_id) {
            order.state = next_state;
        }

        Ok(())
    }

    /// Update order name (owner only)
    pub fn update_order_name(
        &mut self,
        order_id: &str,
        name: String,
        creator_key: &SigningKey,
    ) -> Result<(), String> {
        let (old_state, next_state) = {
            let order = self
                .orders
                .get(order_id)
                .ok_or_else(|| "Order not found".to_string())?;

            // Verify caller is owner
            let params = order.params.to_params()?;
            if creator_key.verifying_key() != params.owner {
                return Err("Only owner can update order name".to_string());
            }

            let new_order = Order {
                name,
                order_version: order.state.order.order.order_version + 1,
            };

            let mut next_state = order.state.clone();
            next_state.order = AuthorizedOrderV1::new(new_order, creator_key);
            (order.state.clone(), next_state)
        };

        self.push_update(order_id, &old_state, &next_state)?;
        if let Some(order) = self.orders.get_mut(order_id) {
            order.state = next_state;
        }

        Ok(())
    }

    /// Delete an order
    pub fn delete_order(&mut self, order_id: &str) {
        self.orders.remove(order_id);
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
