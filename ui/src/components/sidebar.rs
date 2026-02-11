//! Sidebar Component
//!
//! Displays list of pizza orders and allows creating new ones.

use crate::services::AppState;
use dioxus::prelude::*;

#[component]
pub fn Sidebar(
    app_state: Signal<AppState>,
    selected_order_id: Signal<Option<String>>,
    on_select: EventHandler<String>,
    on_new_order: EventHandler<()>,
) -> Element {
    let state = app_state.read();
    let orders: Vec<_> = state.get_orders().into_iter().cloned().collect();
    let selected = selected_order_id.read().clone();
    let is_empty = orders.is_empty();

    rsx! {
        aside { class: "sidebar",
            div { class: "sidebar-header",
                h1 {
                    span { "🍕" }
                    span { "Pizza Orders" }
                }
            }

            div { class: "sidebar-content",
                ul { class: "order-list",
                    for order in orders.iter() {
                        {
                            let order_id = order.id.clone();
                            let order_name = order.state.config.name.clone();
                            let is_active = selected.as_ref() == Some(&order_id);
                            let item_count = order.state.items.items.len();
                            let created = order.state.config.created_at
                                .map(|dt| dt.format("%b %d").to_string())
                                .unwrap_or_else(|| "Unknown".to_string());

                            rsx! {
                                li {
                                    key: "{order_id}",
                                    class: if is_active { "order-item active" } else { "order-item" },
                                    onclick: {
                                        let id = order_id.clone();
                                        move |_| on_select.call(id.clone())
                                    },
                                    div { class: "order-item-name",
                                        "{order_name}"
                                    }
                                    div { class: "order-item-meta",
                                        "{item_count} items · {created}"
                                    }
                                }
                            }
                        }
                    }
                }

                if is_empty {
                    div {
                        style: "padding: 20px; text-align: center; color: rgba(255,255,255,0.5);",
                        "No orders yet"
                    }
                }
            }

            div { class: "sidebar-footer",
                button {
                    class: "btn btn-primary btn-full-width",
                    onclick: move |_| on_new_order.call(()),
                    span { "+" }
                    span { "New Order" }
                }
            }
        }
    }
}
