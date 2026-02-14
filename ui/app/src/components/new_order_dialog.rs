use leptos::prelude::*;
use pizza_common::order_state::*;
use ed25519_dalek::SigningKey;

#[component]
pub fn NewOrderDialog(
    _sk: RwSignal<SigningKey>,
    on_create: Callback<String>,
    on_close: Callback<()>,
) -> impl IntoView {
    let (order_name, set_order_name) = signal(String::new());

    view! {
        <div class="modal-overlay" on:click=move |_| on_close.run(())>
            <div class="modal" on:click=|e| e.stop_propagation()>
                <div class="modal-header">
                    <h3>"Create New Order"</h3>
                </div>

                <form on:submit=move |e| {
                    e.prevent_default();
                    let name = order_name.get();
                    if !name.is_empty() {
                        on_create.run(name);
                    }
                }>
                    <div class="modal-body">
                        <div class="form-group">
                            <label>"Order Name"</label>
                            <input
                                type="text"
                                placeholder="e.g., Pizza for Friday Party"
                                required=true
                                autofocus=true
                                prop:value=order_name
                                on:input=move |e| set_order_name.set(event_target_value(&e))
                            />
                        </div>
                        <p style="color: var(--text-muted); font-size: 0.9rem;">
                            "You'll be the creator of this order and can mark items as paid."
                        </p>
                    </div>

                    <div class="modal-footer">
                        <button
                            class="btn btn-outline"
                            type="button"
                            on:click=move |_| on_close.run(())
                        >
                            "Cancel"
                        </button>
                        <button
                            class="btn btn-primary"
                            type="submit"
                        >
                            "Create Order"
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}
