//! Main Application Component

use crate::components::{InviteDialog, NewOrderDialog, OrderView, Sidebar};
use crate::services::{
    check_url_for_invite, clear_invite_from_url, init_identity, AppState, InviteStore, PizzaInvite,
};
use dioxus::prelude::*;

const MAIN_CSS: &str = include_str!("../../assets/main.css");

/// Main application component
#[component]
pub fn App() -> Element {
    // Initialize identity on first render
    let identity = use_signal(|| init_identity());

    // Application state
    let mut app_state = use_signal(|| AppState::load());

    // Invite store
    let mut invite_store = use_signal(|| InviteStore::load());

    // Currently selected order
    let mut selected_order_id = use_signal(|| None::<String>);

    // Dialog visibility
    let mut show_new_order_dialog = use_signal(|| false);

    // Current invite being shown
    let mut current_invite = use_signal(|| None::<PizzaInvite>);

    // Check for invite in URL on first render
    use_effect(move || {
        if let Some(invite) = check_url_for_invite() {
            clear_invite_from_url();
            current_invite.set(Some(invite));
        }
    });

    // Handle accepting an invite
    let handle_accept_invite = {
        move |invite: PizzaInvite| {
            // Add order to our state if not already present
            let state = app_state.read();
            if state.get_order(&invite.order_id).is_none() {
                // In a real Freenet app, we'd subscribe to the contract here
                // For now, we create a placeholder that would be synced
                drop(state);
                let mut state = app_state.write();
                state.create_order_from_invite(&invite, identity.read().signing_key());
            }

            // Select the order
            selected_order_id.set(Some(invite.order_id.clone()));

            // Remove from pending invites
            invite_store.write().remove_invite(&invite.id);

            // Close dialog
            current_invite.set(None);
        }
    };

    // Handle denying an invite
    let handle_deny_invite = {
        move |invite: PizzaInvite| {
            invite_store.write().remove_invite(&invite.id);
            current_invite.set(None);
        }
    };

    rsx! {
        style { {MAIN_CSS} }
        div { class: "app-container",
            Sidebar {
                app_state: app_state,
                selected_order_id: selected_order_id,
                on_select: move |id: String| {
                    selected_order_id.set(Some(id));
                },
                on_new_order: move |_| {
                    show_new_order_dialog.set(true);
                },
            }

            div { class: "main-content",
                if let Some(order_id) = selected_order_id.read().clone() {
                    OrderView {
                        order_id: order_id,
                        app_state: app_state,
                        identity: identity,
                    }
                } else {
                    div { class: "empty-state",
                        h3 { "No order selected" }
                        p { "Select an order from the sidebar or create a new one" }
                    }
                }
            }

            // New order dialog
            if *show_new_order_dialog.read() {
                NewOrderDialog {
                    on_create: move |name: String| {
                        let id = app_state.write().create_order(name, identity.read().signing_key());
                        selected_order_id.set(Some(id));
                        show_new_order_dialog.set(false);
                    },
                    on_close: move |_| {
                        show_new_order_dialog.set(false);
                    },
                }
            }

            // Invite dialog
            if let Some(invite) = current_invite.read().clone() {
                InviteDialog {
                    invite: invite,
                    on_accept: handle_accept_invite,
                    on_deny: handle_deny_invite,
                }
            }
        }
    }
}
