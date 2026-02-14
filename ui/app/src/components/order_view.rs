use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
#[component]
pub fn OrderView() -> impl IntoView {
    let params = use_params_map();
    let order_id = move || params.get().get("id").unwrap_or_default();

    // Placeholder for actual order data fetching
    // In a real app, this would be a Resource that fetches from AppState/Freenet
    let order_name = move || format!("Order {}", order_id());

    view! {
        <div class="content-header">
            <div>
                <h2>{order_name}</h2>
                <div class="header-meta">
                    "Created 2024-02-14"
                </div>
            </div>
            <div class="header-actions">
                <button class="btn btn-secondary">
                    "Invite other users (will copy link)"
                </button>
            </div>
        </div>

        <div class="content-body">
            <div class="summary-card">
                <div class="summary-row">
                    <span>"Total Items"</span>
                    <span>"0"</span>
                </div>
                <div class="summary-row">
                    <span>"Paid"</span>
                    <span>"$0.00 / $0.00"</span>
                </div>
                <div class="summary-row total">
                    <span>"Outstanding"</span>
                    <span>"$0.00"</span>
                </div>
            </div>

            <div class="your-order-section">
                <h4>"Your Order"</h4>
                <button class="btn btn-secondary">
                    "+ Add Your Order"
                </button>
            </div>

            <h3 style="margin: 20px 0 15px;">"All Orders"</h3>
            <div style="text-align: center; padding: 40px; color: var(--text-muted);">
                "No orders yet. Be the first to add one!"
            </div>
        </div>
    }
}
