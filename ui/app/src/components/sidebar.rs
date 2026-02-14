use leptos::prelude::*;

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
    selected_order_id: Signal<Option<String>>,
    on_select: Callback<String>,
    on_new_order: Callback<()>,
) -> impl IntoView {
    let is_empty = Signal::derive(move || orders.get().is_empty());

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
                            let is_active = move || selected_order_id.get().as_ref() == Some(&order_id_for_active);
                            
                            view! {
                                <li
                                    class=move || if is_active() { "order-item active" } else { "order-item" }
                                    on:click={
                                        let id = order_id.clone();
                                        move |_| on_select.run(id.clone())
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
