use dioxus::prelude::*;
use crate::app::{Contract, Route};
use ed25519_dalek::SigningKey;
use pizza_common::order_state::ItemContentV1;

#[component]
pub fn Sidebar(
    contracts: Signal<Vec<Contract>>,
    sk: Signal<SigningKey>,
    on_new_order: EventHandler<()>,
    selected_order_id: Option<String>,
) -> Element {
    let user_vk = sk.read().verifying_key();

    rsx! {
        aside {
            class: "sidebar",
            div {
                class: "sidebar-header",
                h1 {
                    span { "🍕" }
                    span { "Pizza Orders" }
                }
            }

            div {
                class: "sidebar-content",
                ul {
                    class: "order-list",
                    for contract in contracts.read().iter() {
                        {
                            let order_id = contract.id.clone();
                            let is_active = selected_order_id.as_ref() == Some(&order_id);
                            let url = format!("/order/{}", order_id);
                            let is_admin = contract.parameters.owner == user_vk;

                            let total_cents = contract.state.items.items.iter().filter_map(|ai| {
                                match &ai.item.content {
                                    ItemContentV1::Item { price_cents, .. } => Some(*price_cents),
                                    _ => None,
                                }
                            }).sum::<u64>();

                            let paid_cents = contract.state.items.items.iter().filter_map(|ai| {
                                match &ai.item.content {
                                    ItemContentV1::Item { price_cents, .. } => {
                                        let paid = contract.state.paid.paid.values.get(&ai.item.signed_by).copied().unwrap_or(false);
                                        if paid { Some(*price_cents) } else { None }
                                    }
                                    _ => None,
                                }
                            }).sum::<u64>();

                            let is_fully_paid = total_cents > 0 && paid_cents == total_cents;

                            rsx! {
                                Link {
                                    to: Route::OrderPage { id: order_id },
                                    li {
                                        class: if is_active { "order-item active" } else { "order-item" },
                                        div {
                                            class: "order-item-name",
                                            "{contract.state.order.order.name}"
                                            if is_admin {
                                                span {
                                                    class: "status-badge",
                                                    style: "color: red; margin-left: 8px; font-size: 0.7em; padding: 2px 6px;",
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
                                            "{contract.state.items.items.len()} items · {contract.parameters.created_at.to_rfc3339()}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if contracts.read().is_empty() {
                    div {
                        style: "padding: 20px; text-align: center; color: rgba(255,255,255,0.5);",
                        "No orders yet"
                    }
                }
            }

            div {
                class: "sidebar-footer",
                button {
                    class: "btn btn-primary btn-full-width",
                    onclick: move |_| on_new_order.call(()),
                    span { "+ New Order" }
                }
            }
        }
    }
}
