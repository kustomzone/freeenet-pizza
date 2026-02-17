use crate::components::{OrderSettings, OrderSettingsForm};
use dioxus::prelude::*;
use ed25519_dalek::SigningKey;

#[component]
pub fn NewOrderDialog(
    sk: Signal<SigningKey>,
    on_create: EventHandler<OrderSettings>,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal",
                onclick: |e| e.stop_propagation(),
                div {
                    class: "modal-header",
                    h3 { "Create New Order" }
                }

                OrderSettingsForm {
                    submit_label: "Create Order".to_string(),
                    on_submit: move |settings: OrderSettings| {
                        on_create.call(settings);
                    },
                    on_cancel: move |_| on_close.call(()),
                }

                p {
                    style: "padding: 0 24px 16px; color: var(--text-muted); font-size: 0.9rem;",
                    "You'll be the admin of this order and can mark items as paid."
                }
            }
        }
    }
}
