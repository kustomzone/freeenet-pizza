use leptos::prelude::*;
use crate::Contract;
use ed25519_dalek::SigningKey;
use pizza_common::order_state::ItemContentV1;

#[component]
pub fn Sidebar(
    contracts: RwSignal<Vec<Contract>>,
    sk: RwSignal<SigningKey>,
    on_new_order: Callback<()>,
    selected_order_id: Option<String>,
) -> impl IntoView {
    let is_empty = Signal::derive(move || contracts.with(|c| c.is_empty()));
    let user_vk = move || sk.get().verifying_key();
    // let navigate = use_navigate();

    view! {
        <aside class="sidebar">
            <div class="sidebar-header">
                <h1>
                    <span>"🍕"</span>
                    <span>"Pizza Orders"</span>
                </h1>
            </div>

            <div class="sidebar-content">
                <ul class="order-list">
                    <For
                        each=move || contracts.get()
                        key=|contract: &Contract| contract.id.clone()
                        children=move |contract| {
                            let order_id = contract.id.clone();
                            let order_id_for_active = order_id.clone();
                            let is_active = selected_order_id.as_ref() == Some(&order_id_for_active);
                            // let navigate = navigate.clone();
                            let order_id_for_nav = order_id.clone();
                            let url = vec!["/order/", &order_id_for_nav].join("");

                            let is_admin = contract.parameters.owner == user_vk();

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

                            view! {
                                <a href=url>
                                    <li
                                        class=move || if is_active { "order-item active" } else { "order-item" }
                                        on:click=move |_| {
                                            // navigate(&format!("/order/{}", order_id_for_nav), Default::default());
                                        }
                                    >
                                        <div class="order-item-name">
                                            {contract.state.order.order.name.clone()}
                                            {if is_admin {
                                                view! { <span class="status-badge" style="color: red; margin-left: 8px; font-size: 0.7em; padding: 2px 6px;">"Admin"</span> }.into_any()
                                            } else {
                                                view! {}.into_any()
                                            }}
                                            {if is_fully_paid {
                                                view! { <span class="status-badge paid" style="margin-left: 8px; font-size: 0.7em; padding: 2px 6px;">"Paid"</span> }.into_any()
                                            } else {
                                                view! {}.into_any()
                                            }}
                                        </div>
                                        <div class="order-item-meta">
                                            {contract.state.items.items.len()} " items · " {contract.parameters.created_at.to_rfc3339()}
                                        </div>
                                    </li>
                                </a>
                            }
                        }
                    />
                </ul>

                <Show when=move || is_empty.get()>
                    <div style="padding: 20px; text-align: center; color: rgba(255,255,255,0.5);">
                        "No orders yet"
                    </div>
                </Show>
            </div>

            <div class="sidebar-footer">
                <button
                    class="btn btn-primary btn-full-width"
                    on:click=move |_| on_new_order.run(())
                >
                    <span>"+ New Order"</span>
                </button>
            </div>
        </aside>
    }
}
