use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use leptos_router::hooks::use_navigate;

#[derive(Debug, Clone, PartialEq)]
pub struct PizzaOrder {
    pub id: String,
    pub name: String,
    pub item_count: usize,
    pub created_at: String,
}

#[component]
pub fn Sidebar(
    orders: Signal<Vec<PizzaOrder>>,
    on_new_order: Callback<()>,
) -> impl IntoView {
    let is_empty = Signal::derive(move || orders.get().is_empty());
    let params = use_params_map();
    let selected_order_id = move || params.get().get("id");
    let navigate = use_navigate();

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
                        each=move || orders.get()
                        key=|order| order.id.clone()
                        children=move |order| {
                            let order_id = order.id.clone();
                            let order_id_for_active = order_id.clone();
                            let is_active = move || selected_order_id().as_ref() == Some(&order_id_for_active);
                            let navigate = navigate.clone();
                            let order_id_for_nav = order_id.clone();
                            
                            view! {
                                <li
                                    class=move || if is_active() { "order-item active" } else { "order-item" }
                                    on:click=move |_| {
                                        navigate(&format!("/order/{}", order_id_for_nav), Default::default());
                                    }
                                >
                                    <div class="order-item-name">
                                        {order.name.clone()}
                                    </div>
                                    <div class="order-item-meta">
                                        {order.item_count} " items · " {order.created_at.clone()}
                                    </div>
                                </li>
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
