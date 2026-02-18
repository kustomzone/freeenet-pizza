use crate::api::{ConnectionStatus, CONNECTION_STATUS};
use crate::app::{Contract, Route};
use crate::util::format_utc_as_full_datetime;
use dioxus::prelude::*;
use ed25519_dalek::SigningKey;
use pizza_common::order_state::ItemContentV1;
use std::collections::HashMap;

#[component]
pub fn Sidebar(
    contracts: Signal<HashMap<String, Contract>>,
    sk: Signal<SigningKey>,
    on_new_order: EventHandler<()>,
    on_delete: EventHandler<String>,
    selected_order_id: Option<String>,
    #[props(default = false)] loading: bool,
) -> Element {
    let user_vk = sk.read().verifying_key();
    let connection_status = CONNECTION_STATUS.read();
    let mut sidebar_open = use_signal(|| false);

    let status_indicator = match &*connection_status {
        ConnectionStatus::Connected => rsx! {
            span {
                class: "status-indicator connected",
                title: "Connected to Freenet node",
                "●"
            }
        },
        ConnectionStatus::Connecting => rsx! {
            span {
                class: "status-indicator connecting",
                title: "Connecting to Freenet node...",
                "●"
            }
        },
        ConnectionStatus::Disconnected => rsx! {
            span {
                class: "status-indicator disconnected",
                title: "Disconnected from Freenet node",
                "●"
            }
        },
        ConnectionStatus::Error(e) => rsx! {
            span {
                class: "status-indicator error",
                title: "Connection error: {e}",
                "❌"
            }
        },
    };

    rsx! {
        // Hamburger button for mobile (only visible when sidebar is closed)
        button {
            class: if sidebar_open() { "hamburger-btn hamburger-hidden" } else { "hamburger-btn" },
            onclick: move |_| sidebar_open.set(true),
            span { class: "hamburger-line" }
            span { class: "hamburger-line" }
            span { class: "hamburger-line" }
        }

        // Overlay for mobile when sidebar is open
        if sidebar_open() {
            div {
                class: "sidebar-overlay",
                onclick: move |_| sidebar_open.set(false),
            }
        }

        aside {
            class: if sidebar_open() { "sidebar sidebar-open" } else { "sidebar" },
            div {
                class: "sidebar-header",
                // Close button for mobile (inside sidebar)
                button {
                    class: "sidebar-close-btn",
                    onclick: move |_| sidebar_open.set(false),
                    "✕"
                }
                Link {
                    to: Route::AboutPageRoute {},
                    class: "sidebar-title-link",
                    onclick: move |_| sidebar_open.set(false),
                    h1 {
                        span { "🍕" }
                        span { "Pizza Orders" }
                    }
                }
                {status_indicator}
            }

            div {
                class: "sidebar-content",
                ul {
                    class: "order-list",
                    for (id, contract) in contracts.read().iter() {
                        {
                            let order_id = id.clone();
                            let is_active = selected_order_id.as_ref() == Some(&order_id);
                            let is_admin = contract.parameters.owner == user_vk;

                            let total_cents = contract.state.items.items.iter().filter_map(|ai| {
                                match &ai.item.content {
                                    ItemContentV1::Item { price_cents, .. } => Some(price_cents),
                                    _ => None,
                                }
                            }).sum::<u64>();

                            let paid_cents = contract.state.items.items.iter().filter_map(|ai| {
                                match &ai.item.content {
                                    ItemContentV1::Item { price_cents, .. } => {
                                        let paid = contract.state.paid.paid.values.get(&ai.item.signed_by).copied().unwrap_or(false);
                                        if paid { Some(price_cents) } else { None }
                                    }
                                    _ => None,
                                }
                            }).sum::<u64>();

                            let is_fully_paid = total_cents > 0 && paid_cents == total_cents;

                            let delete_id = id.clone();
                            rsx! {
                                li {
                                    class: if is_active { "order-item active" } else { "order-item" },
                                    Link {
                                        to: Route::OrderPage { id: order_id },
                                        onclick: move |_| sidebar_open.set(false),
                                        div {
                                            class: "order-item-content",
                                            div {
                                                class: "order-item-name",
                                                "{contract.state.order.order.name}"
                                                if is_admin {
                                                    span {
                                                        class: "status-badge admin",
                                                        style: "margin-left: 8px; font-size: 0.7em; padding: 2px 6px;",
                                                        "Admin"
                                                    }
                                                }
                                                if is_fully_paid {
                                                    span {
                                                        class: "status-badge paid",
                                                        style: "margin-left: 8px; font-size: 0.7em; padding: 2px 6px;",
                                                        "Paid"
                                                    }
                                                }
                                            }
                                            div {
                                                class: "order-item-meta",
                                                "{contract.state.items.items.len()} items · {format_utc_as_full_datetime(contract.parameters.created_at.timestamp_millis())}"
                                            }
                                        }
                                    }
                                    button {
                                        class: "order-item-delete",
                                        title: "Remove from list",
                                        onclick: move |e| {
                                            e.stop_propagation();
                                            on_delete.call(delete_id.clone());
                                        },
                                        "×"
                                    }
                                }
                            }
                        }
                    }
                }

                if contracts.read().is_empty() {
                    div {
                        style: "padding: 20px; text-align: center; color: rgba(255,255,255,0.5);",
                        if loading {
                            div {
                                class: "sidebar-loading",
                                div {
                                    class: "loading-spinner-small",
                                }
                                span { "Loading orders..." }
                            }
                        } else {
                            "No orders yet"
                        }
                    }
                }
            }

            div {
                class: "sidebar-footer",
                Link {
                    to: Route::AboutPageRoute {},
                    class: "sidebar-about-link",
                    onclick: move |_| sidebar_open.set(false),
                    "About"
                }
                button {
                    class: "btn btn-primary btn-full-width",
                    onclick: move |_| on_new_order.call(()),
                    span { "+ New Order" }
                }
            }
        }
    }
}
