use dioxus::prelude::*;
use crate::services::{BaseService, Contract};
use ed25519_dalek::{SigningKey, VerifyingKey};
use pizza_common::FullOrderStateV1Delta;
use pizza_common::order_state::{ItemContentV1, ItemV1, AuthorizedItemV1};
use pizza_common::util::{format_price, parse_price};

#[component]
pub fn YourOrderSection(
    id: String,
    contract: Contract,
    user_vk: VerifyingKey,
    sk: SigningKey,
) -> Element {
    let base = use_context::<BaseService>();
    
    // Form signals
    let mut display_name = use_signal(String::new);
    let mut order_text = use_signal(String::new);
    let mut price_input = use_signal(String::new);
    let mut price_error = use_signal(|| Option::<String>::None);
    let mut show_add_form = use_signal(|| false);
    let mut edit_mode = use_signal(|| false);

    let user_item = contract.state.items.items.iter().find_map(|ai| {
        if ai.item.signed_by == user_vk {
            match &ai.item.content {
                ItemContentV1::Item { display_name, order, price_cents } => {
                    Some((display_name.clone(), order.clone(), *price_cents))
                }
                _ => None,
            }
        } else {
            None
        }
    });

    let mut handle_add_item = {
        let id = id.clone();
        let base = base.clone();
        let sk = sk.clone();
        let contract = contract.clone();
        move |_e: FormEvent| {
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
            let user_key = sk.clone();
            let user_vk_val = user_key.verifying_key();

            let next_version = contract.state.items.items.iter()
                .find(|it| it.item.signed_by == user_vk_val)
                .map(|it| it.item.version + 1)
                .unwrap_or(1);

            let item_val = ItemV1 {
                signed_by: user_vk_val,
                owner_sign: false,
                version: next_version,
                content: ItemContentV1::Item {
                    display_name: dn,
                    order: ot,
                    price_cents: price,
                },
            };
            let delta = FullOrderStateV1Delta {
                order: None,
                items: Some(vec![AuthorizedItemV1::new(item_val.clone(), &user_key)]),
                paid: None,
                version: None,
            };
            let _ = base.publish_delta(id.clone(), delta);

            display_name.set(String::new());
            order_text.set(String::new());
            price_input.set(String::new());
            show_add_form.set(false);
        }
    };

    let mut handle_edit_item = {
        let id = id.clone();
        let base = base.clone();
        let sk = sk.clone();
        let contract = contract.clone();
        move |_e: FormEvent| {
            let dn = display_name.read().clone();
            let ot = order_text.read().clone();
            let pi = price_input.read().clone();

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
            let user_key = sk.clone();
            let user_vk_val = user_key.verifying_key();

            if let Some(pos) = contract.state.items.items.iter().position(|it| it.item.signed_by == user_vk_val) {
                let existing = &contract.state.items.items[pos];
                
                let mut final_dn = dn;
                let mut final_ot = ot;
                
                if final_dn.is_empty() {
                    if let ItemContentV1::Item { display_name, .. } = &existing.item.content {
                        final_dn = display_name.clone();
                    }
                }
                if final_ot.is_empty() {
                    if let ItemContentV1::Item { order, .. } = &existing.item.content {
                        final_ot = order.clone();
                    }
                }

                let new_item = ItemV1 {
                    signed_by: user_vk_val,
                    owner_sign: false,
                    version: existing.item.version + 1,
                    content: ItemContentV1::Item {
                        display_name: final_dn,
                        order: final_ot,
                        price_cents: price,
                    },
                };
                let delta = FullOrderStateV1Delta {
                    order: None,
                    items: Some(vec![AuthorizedItemV1::new(new_item.clone(), &user_key)]),
                    paid: None,
                    version: None,
                };
                let _ = base.publish_delta(id.clone(), delta);
            }
            edit_mode.set(false);
        }
    };

    let handle_delete_item = {
        let id = id.clone();
        let base = base.clone();
        let sk = sk.clone();
        let contract = contract.clone();
        move || {
            let user_vk_val = user_vk;
            let owner_sk = sk.clone();
            if let Some(item) = contract.state.items.items.iter().find(|it| it.item.signed_by == user_vk_val) {
                let new_item = ItemV1 {
                    signed_by: item.item.signed_by,
                    owner_sign: false,
                    content: ItemContentV1::Deleted {},
                    version: item.item.version + 1,
                };
                let delta = FullOrderStateV1Delta {
                    order: None,
                    items: Some(vec![AuthorizedItemV1::new(new_item.clone(), &owner_sk)]),
                    paid: None,
                    version: None,
                };
                let _ = base.publish_delta(id.clone(), delta);
            }
        }
    };

    rsx! {
        div {
            class: "your-order-section",
            h4 { "Your Order" }
            {
                if let Some(item) = user_item {
                    if edit_mode() {
                        rsx! {
                            form {
                                onsubmit: move |e| handle_edit_item(e),
                                div {
                                    class: "form-row",
                                    div {
                                        class: "form-group",
                                        label { "Display Name" }
                                        input {
                                            r#type: "text",
                                            value: "{display_name}",
                                            oninput: move |e| display_name.set(e.value())
                                        }
                                    }
                                    div {
                                        class: "form-group",
                                        label { "Price" }
                                        input {
                                            r#type: "text",
                                            value: "{price_input}",
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
                                                class: "error-message",
                                                style: "color: var(--error-color, red); font-size: 0.8em; margin-top: 4px;",
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
                                        oninput: move |e| order_text.set(e.value()),
                                    }
                                }
                                div {
                                    style: "display: flex; gap: 10px;",
                                    button {
                                        class: "btn btn-primary",
                                        r#type: "submit",
                                        "Save Changes"
                                    }
                                    button {
                                        class: "btn btn-outline",
                                        r#type: "button",
                                        onclick: move |_| edit_mode.set(false),
                                        "Cancel"
                                    }
                                }
                            }
                        }
                    } else {
                        let is_paid = contract.state.paid.paid.values.get(&user_vk).copied().unwrap_or(false);
                        rsx! {
                            div {
                                style: "display: flex; justify-content: space-between; align-items: start;",
                                div {
                                    p {
                                        strong { "{item.0}" }
                                        " - {item.1}"
                                    }
                                    p {
                                        style: "color: var(--text-muted);",
                                        "Price: {format_price(item.2)}"
                                        if is_paid {
                                            span {
                                                class: "status-badge paid",
                                                style: "margin-left: 10px;",
                                                "Paid"
                                            }
                                        }
                                    }
                                }
                                div {
                                    class: "item-actions",
                                    button {
                                        class: "btn btn-small btn-outline",
                                        onclick: move |_| {
                                            display_name.set(item.0.clone());
                                            order_text.set(item.1.clone());
                                            price_input.set(format_price(item.2));
                                            price_error.set(None);
                                            edit_mode.set(true);
                                        },
                                        "Edit"
                                    }
                                    button {
                                        class: "btn btn-small btn-outline",
                                        onclick: move |_| handle_delete_item(),
                                        "Remove"
                                    }
                                }
                            }
                        }
                    }
                } else {
                    if show_add_form() {
                        rsx! {
                            form {
                                onsubmit: move |e| handle_add_item(e),
                                div {
                                    class: "form-row",
                                    div {
                                        class: "form-group",
                                        label { "Your Name" }
                                        input {
                                            r#type: "text",
                                            value: "{display_name}",
                                            placeholder: "e.g., John",
                                            required: true,
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
                                                class: "error-message",
                                                style: "color: var(--error-color, red); font-size: 0.8em; margin-top: 4px;",
                                                "{err}"
                                            }
                                        }
                                    }
                                }
                                div {
                                    class: "form-group",
                                    label { "What would you like?" }
                                    textarea {
                                        value: "{order_text}",
                                        placeholder: "e.g., 1x Margherita, extra cheese",
                                        required: true,
                                        oninput: move |e| order_text.set(e.value())
                                    }
                                }
                                div {
                                    style: "display: flex; gap: 10px;",
                                    button {
                                        class: "btn btn-primary",
                                        r#type: "submit",
                                        "Add My Order"
                                    }
                                    button {
                                        class: "btn btn-outline",
                                        r#type: "button",
                                        onclick: move |_| show_add_form.set(false),
                                        "Cancel"
                                    }
                                }
                            }
                        }
                    } else {
                        rsx! {
                            button {
                                class: "btn btn-secondary",
                                onclick: move |_| {
                                    price_error.set(None);
                                    show_add_form.set(true);
                                },
                                "+ Add Your Order"
                            }
                        }
                    }
                }
            }
        }
    }
}
