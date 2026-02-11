//! New Order Dialog Component
//!
//! Modal dialog for creating a new pizza order.

use dioxus::prelude::*;

#[component]
pub fn NewOrderDialog(
    on_create: EventHandler<String>,
    on_close: EventHandler<()>,
) -> Element {
    let mut order_name = use_signal(|| String::new());

    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| on_close.call(()),

            div {
                class: "modal",
                onclick: |e| e.stop_propagation(),

                div { class: "modal-header",
                    h3 { "Create New Order" }
                }

                form {
                    onsubmit: move |e: Event<FormData>| {
                        e.prevent_default();
                        let name = order_name.read().clone();
                        if !name.is_empty() {
                            on_create.call(name);
                        }
                    },

                    div { class: "modal-body",
                        div { class: "form-group",
                            label { "Order Name" }
                            input {
                                r#type: "text",
                                placeholder: "e.g., Pizza for Friday Party",
                                required: true,
                                autofocus: true,
                                value: "{order_name}",
                                oninput: move |e| order_name.set(e.value()),
                            }
                        }
                        p {
                            style: "color: var(--text-muted); font-size: 0.9rem;",
                            "You'll be the creator of this order and can mark items as paid."
                        }
                    }

                    div { class: "modal-footer",
                        button {
                            class: "btn btn-outline",
                            r#type: "button",
                            onclick: move |_| on_close.call(()),
                            "Cancel"
                        }
                        button {
                            class: "btn btn-primary",
                            r#type: "submit",
                            "Create Order"
                        }
                    }
                }
            }
        }
    }
}
