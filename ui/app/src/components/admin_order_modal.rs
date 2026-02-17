use dioxus::prelude::*;
use ed25519_dalek::{SigningKey, VerifyingKey};
use log::info;
use pizza_common::FullOrderStateV1Delta;
use pizza_common::order_state::{ItemContentV1, ItemV1, AuthorizedItemV1};
use pizza_common::util::{format_price, parse_price};
use crate::services::BaseService;

#[derive(Clone, PartialEq)]
pub enum AdminOrderMode {
    Add,
    Edit {
        target_vk: VerifyingKey,
        initial_name: String,
        initial_order: String,
        initial_price: u64,
    },
}

#[component]
pub fn AdminOrderModal(
    contract_id: String,
    owner_sk: SigningKey,
    mode: AdminOrderMode,
    on_close: EventHandler<()>,
) -> Element {
    let base = use_context::<BaseService>();

    let (is_edit, initial_name, initial_order, initial_price) = match &mode {
        AdminOrderMode::Add => (false, String::new(), String::new(), String::new()),
        AdminOrderMode::Edit { initial_name, initial_order, initial_price, .. } => {
            (true, initial_name.clone(), initial_order.clone(), format_price(*initial_price))
        }
    };

    let mut display_name = use_signal(|| initial_name);
    let mut order_text = use_signal(|| initial_order);
    let mut price_input = use_signal(|| initial_price);
    let mut price_error = use_signal(|| Option::<String>::None);

    let title = if is_edit { "Edit Order" } else { "Add Order (Admin)" };
    let submit_label = if is_edit { "Save Changes" } else { "Add Order" };

    let handle_submit = {
        let contract_id = contract_id.clone();
        let base = base.clone();
        let owner_sk = owner_sk.clone();
        let mode = mode.clone();
        move || {
            let dn = display_name.read().clone();
            let ot = order_text.read().clone();
            let pi = price_input.read().clone();

            if dn.is_empty() || ot.is_empty() {
                return;
            }

            let price = match parse_price(&pi) {
                Ok(p) => {
                    price_error.set(None);
                    p
                }
                Err(e) => {
                    price_error.set(Some(e));
                    return;
                }
            };

            let delta = match &mode {
                AdminOrderMode::Add => {
                    // Generate a new random VK for this order
                    let mut rng = rand::thread_rng();
                    let new_sk = SigningKey::generate(&mut rng);
                    let new_vk = new_sk.verifying_key();

                    let new_item = ItemV1 {
                        signed_by: new_vk,
                        owner_sign: true,
                        version: 1,
                        content: ItemContentV1::Item {
                            display_name: dn,
                            order: ot,
                            price_cents: price,
                        },
                    };
                    FullOrderStateV1Delta {
                        order: None,
                        items: Some(vec![AuthorizedItemV1::new(new_item, &owner_sk)]),
                        paid: None,
                        version: None,
                    }
                }
                AdminOrderMode::Edit { target_vk, .. } => {
                    let target_vk = *target_vk;

                    // Get current state from cache
                    let current_state = match base.get_contract_cached(contract_id.clone()) {
                        Some(contract) => contract,
                        None => return,
                    };

                    // Find the existing item
                    let existing = match current_state.state.items.items.iter().find(|it| it.item.signed_by == target_vk) {
                        Some(e) => e,
                        None => {
                            info!("AdminOrderModal: existing item not found");
                            return;
                        }
                    };

                    let new_item = ItemV1 {
                        signed_by: target_vk,
                        owner_sign: true,
                        version: existing.item.version + 1,
                        content: ItemContentV1::Item {
                            display_name: dn,
                            order: ot,
                            price_cents: price,
                        },
                    };
                    FullOrderStateV1Delta {
                        order: None,
                        items: Some(vec![AuthorizedItemV1::new(new_item, &owner_sk)]),
                        paid: None,
                        version: None,
                    }
                }
            };

            // Publish delta synchronously - spawn the async work but don't close until we've
            // at least started the publish. We use spawn_forever to ensure the task isn't
            // cancelled when the component unmounts.
            let base = base.clone();
            let id = contract_id.clone();

            // Use wasm_bindgen_futures to spawn a task that won't be cancelled
            wasm_bindgen_futures::spawn_local(async move {
                let result = base.publish_delta(id, delta).await;
            });

            on_close.call(());
        }
    };
    let mut handle_submit = handle_submit;

    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal",
                onclick: move |e| e.stop_propagation(),
                div {
                    class: "modal-header",
                    h3 { "{title}" }
                }
                div {
                    class: "modal-body",
                    form {
                        onsubmit: move |e: FormEvent| {
                            e.prevent_default();
                        },
                        div {
                            class: "form-row",
                            div {
                                class: "form-group",
                                label { "Display Name" }
                                input {
                                    r#type: "text",
                                    value: "{display_name}",
                                    placeholder: "e.g., John",
                                    oninput: move |e| display_name.set(e.value())
                                }
                            }
                            div {
                                class: "form-group",
                                label { "Price" }
                                input {
                                    r#type: "text",
                                    value: "{price_input}",
                                    placeholder: "e.g., 12.50",
                                    oninput: move |e| {
                                        let val = e.value();
                                        price_input.set(val.clone());
                                        if let Err(err) = parse_price(&val) {
                                            price_error.set(Some(err));
                                        } else {
                                            price_error.set(None);
                                        }
                                    }
                                }
                                if let Some(err) = price_error() {
                                    div {
                                        style: "color: var(--primary-color); font-size: 0.8em; margin-top: 4px;",
                                        "{err}"
                                    }
                                }
                            }
                        }
                        div {
                            class: "form-group",
                            label { "Order" }
                            textarea {
                                value: "{order_text}",
                                placeholder: "e.g., 1x Margherita, extra cheese",
                                oninput: move |e| order_text.set(e.value()),
                            }
                        }
                    }
                }
                div {
                    class: "modal-footer",
                    button {
                        class: "btn btn-outline",
                        onclick: move |_| on_close.call(()),
                        "Cancel"
                    }
                    button {
                        class: "btn btn-primary",
                        r#type: "button",
                        onclick: move |_| {
                            handle_submit();
                        },
                        "{submit_label}"
                    }
                }
            }
        }
    }
}
