use crate::order_state::OrderParametersV1;
use crate::util::sign_struct;
use crate::util::{truncated_base64, verify_struct};
use crate::{FullOrderStateV1, UserId, UserIdKey};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use freenet_scaffold::util::{fast_hash, FastHash};
use freenet_scaffold::ComposableState;
use serde::{Deserialize, Serialize};
use std::fmt;
use crate::order_state::items::ItemContentV1::Item;

pub const MAX_SUB_ELEMENTS: usize = 100;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct ItemsV1 {
    pub items: Vec<AuthorizedItemV1>,
}

impl ComposableState for ItemsV1 {
    type ParentState = FullOrderStateV1;
    type Summary = Vec<ItemId>;
    type Delta = Vec<AuthorizedItemV1>;
    type Parameters = OrderParametersV1;

    fn verify(
        &self,
        parent_state: &Self::ParentState,
        parameters: &Self::Parameters,
    ) -> Result<(), String> {
        for item in &self.items {
            let verifying_key = if item.item.owner_sign {
                &parameters.owner
            } else {
                &item.item.signed_by
            };

            if item.validate(verifying_key).is_err() {
                return Err(format!("Invalid message signature: id:{:?}", item.id()));
            }
        }

        Ok(())
    }

    fn summarize(
        &self,
        _parent_state: &Self::ParentState,
        _parameters: &Self::Parameters,
    ) -> Self::Summary {
        self.items.iter().map(|m| m.id()).collect()
    }

    fn delta(
        &self,
        _parent_state: &Self::ParentState,
        _parameters: &Self::Parameters,
        old_state_summary: &Self::Summary,
    ) -> Option<Self::Delta> {
        let delta: Vec<AuthorizedItemV1> = self
            .items
            .iter()
            .filter(|m| !old_state_summary.contains(&m.id()))
            .cloned()
            .collect();
        if delta.is_empty() {
            None
        } else {
            Some(delta)
        }
    }

    fn apply_delta(
        &mut self,
        parent_state: &Self::ParentState,
        parameters: &Self::Parameters,
        delta: &Option<Self::Delta>,
    ) -> Result<(), String> {
        if let Some(delta) = delta {
            let mut items_c = self.items.clone();

            for incoming in delta {
                // Validate signature against appropriate key
                let verifying_key = if incoming.item.owner_sign {
                    &parameters.owner
                } else {
                    &incoming.item.signed_by
                };
                if incoming.validate(verifying_key).is_err() {
                    return Err(format!("Invalid item signature: id:{:?}", incoming.id()));
                }

                // Find existing item from the same signer (signed_by)
                if let Some(pos) = self
                    .items
                    .iter()
                    .position(|it| it.item.signed_by == incoming.item.signed_by)
                {
                    // Replace if incoming version is newer
                    if incoming.item.version > items_c[pos].item.version {
                        items_c[pos] = incoming.clone();
                    }
                } else {
                    // No existing item for this signer – insert
                    items_c.push(incoming.clone());
                }
            }

            self.items = items_c
        }

        /*let max_recent_messages = parent_state.configuration.configuration.max_recent_messages;
        let max_message_size = parent_state.configuration.configuration.max_message_size;
        let privacy_mode = &parent_state.configuration.configuration.privacy_mode;
        let current_secret_version = parent_state.secrets.current_version;

        // Validate message constraints before adding
        if let Some(delta) = delta {
            for msg in delta {
                let content = &msg.item.content;

                match content {
                    ItemActionBody::Private { secret_version, .. } => {
                        // In private mode, verify secret version matches current
                        if *privacy_mode == PrivacyMode::Private {
                            if *secret_version != current_secret_version {
                                return Err(format!(
                                    "Private message secret version {} does not match current version {}",
                                    secret_version, current_secret_version
                                ));
                            }
                        }

                        // Verify all current members have encrypted blobs for this version
                        let members = parent_state.members.members_by_member_id();
                        if !parent_state.secrets.has_complete_distribution(&members) {
                            return Err(
                                "Cannot accept private messages: incomplete secret distribution"
                                    .to_string(),
                            );
                        }
                    }
                    ItemActionBody::Public { .. } => {
                        // In private mode, reject ALL public messages including actions
                        // Privacy is a layer - everything in a private room must be encrypted
                        if *privacy_mode == PrivacyMode::Private {
                            return Err("Cannot send public messages in private room".to_string());
                        }
                    }
                }
            }

            // Deduplicate by message ID to prevent duplicate messages from race conditions
            let existing_ids: std::collections::HashSet<_> =
                self.items.iter().map(|m| m.id()).collect();
            self.items.extend(
                delta
                    .iter()
                    .filter(|msg| !existing_ids.contains(&msg.id()))
                    .cloned(),
            );
        }

        // Always enforce message constraints
        // Ensure there are no messages over the size limit
        self.items
            .retain(|m| m.item.content.content_len() <= max_message_size);

        // Ensure all messages are signed by a valid member or the room owner, remove if not
        let members_by_id = parent_state.members.members_by_member_id();
        let owner_id = MemberId::from(&parameters.owner);
        self.items.retain(|m| {
            members_by_id.contains_key(&m.item.author) || m.item.author == owner_id
        });

        // Sort messages by time, with MessageId as secondary sort for deterministic ordering
        // (CRDT convergence requirement - without this, ties produce non-deterministic order)
        self.items.sort_by(|a, b| {
            a.item
                .time
                .cmp(&b.item.time)
                .then_with(|| a.id().cmp(&b.id()))
        });

        // Remove oldest messages if there are too many
        if self.items.len() > max_recent_messages {
            self.items
                .drain(0..self.items.len() - max_recent_messages);
        }

        // Rebuild computed state from action messages
        self.rebuild_actions_state();*/

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct ItemV1 {
    /// Who signed this version (owner or creator)
    pub signed_by: VerifyingKey,
    pub owner_sign: bool,
    pub version: u64,
    pub content: ItemContentV1,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ItemContentV1 {
    Item {
        /// User's display name for this order
        display_name: String,
        /// Description of what they're ordering (e.g., "2x Margherita")
        order: String,
        /// Price in cents (to avoid floating point issues)
        price_cents: u64,
    },
    Deleted {
    }
}

impl Default for ItemV1 {
    fn default() -> Self {
        Self {
            signed_by: Default::default(),
            owner_sign: false,
            version: 0,
            content: {
                Item {
                    display_name: "".to_string(),
                    order: "".to_string(),
                    price_cents: 0,
                }
            },
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthorizedItemV1 {
    pub item: ItemV1,
    pub signature: Signature,
}

impl fmt::Debug for AuthorizedItemV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthorizedItem")
            .field("message", &self.item)
            .field(
                "signature",
                &format_args!("{}", truncated_base64(self.signature.to_bytes())),
            )
            .finish()
    }
}

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize, Clone, Debug, Ord, PartialOrd)]
pub struct ItemId(pub FastHash);

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl AuthorizedItemV1 {
    pub fn new(message: ItemV1, signing_key: &SigningKey) -> Self {
        Self {
            item: message.clone(),
            signature: sign_struct(&message, signing_key),
        }
    }

    /// Create an AuthorizedItemV1 with a pre-computed signature.
    /// Use this when signing is done externally (e.g., via delegate).
    pub fn with_signature(message: ItemV1, signature: Signature) -> Self {
        Self { item: message, signature }
    }

    pub fn validate(
        &self,
        verifying_key: &VerifyingKey,
    ) -> Result<(), ed25519_dalek::SignatureError> {
        verify_struct(&self.item, &self.signature, verifying_key)
    }

    pub fn id(&self) -> ItemId {
        ItemId(fast_hash(&self.signature.to_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_apply_delta_version_replacement() {
        let mut rng = OsRng;
        let owner_signing_key = SigningKey::generate(&mut rng);
        let owner_verifying_key = owner_signing_key.verifying_key();

        let user_signing_key = SigningKey::generate(&mut rng);
        let user_id = user_signing_key.verifying_key();

        let parameters = OrderParametersV1 {
            owner: owner_verifying_key,
            created_at: Utc::now(),
        };
        let parent_state = FullOrderStateV1::default();

        let mut items_state = ItemsV1::default();

        // 1. Initial item (version 1)
        let item_v1 = ItemV1 {
            signed_by: user_id,
            owner_sign: false,
            version: 1,
            content: ItemContentV1::Item {
                display_name: "User".to_string(),
                order: "Pizza V1".to_string(),
                price_cents: 1000,
            },
        };
        let auth_item_v1 = AuthorizedItemV1::new(item_v1, &user_signing_key);
        items_state.apply_delta(&parent_state, &parameters, &Some(vec![auth_item_v1.clone()])).unwrap();

        assert_eq!(items_state.items.len(), 1);
        assert_eq!(items_state.items[0].item.version, 1);

        // 2. Apply item with version 2 (should replace)
        let item_v2 = ItemV1 {
            signed_by: user_id,
            owner_sign: false,
            version: 2,
            content: ItemContentV1::Item {
                display_name: "User".to_string(),
                order: "Pizza V2".to_string(),
                price_cents: 1200,
            },
        };
        let auth_item_v2 = AuthorizedItemV1::new(item_v2, &user_signing_key);
        items_state.apply_delta(&parent_state, &parameters, &Some(vec![auth_item_v2.clone()])).unwrap();

        assert_eq!(items_state.items.len(), 1);
        assert_eq!(items_state.items[0].item.version, 2);
        if let ItemContentV1::Item { order, .. } = &items_state.items[0].item.content {
            assert_eq!(order, "Pizza V2");
        } else {
            panic!("Wrong content type");
        }

        // 3. Apply item with version 1 again (should NOT replace)
        items_state.apply_delta(&parent_state, &parameters, &Some(vec![auth_item_v1])).unwrap();
        assert_eq!(items_state.items.len(), 1);
        assert_eq!(items_state.items[0].item.version, 2);
    }
}
