use dioxus::prelude::*;
use crate::Contract;
use ed25519_dalek::{SigningKey, VerifyingKey};
use pizza_common::{ComposableState, FullOrderStateV1Delta};
use pizza_common::order_state::{ItemContentV1, ItemV1, AuthorizedItemV1, Paid, AuthorizedPaidV1};

#[component]
pub fn OrderViewComponent(
    mut contracts: Signal<Vec<Contract>>,
    sk: Signal<SigningKey>,
    id: String,
) -> Element {
    let order_id = id;

    let contract = contracts.read().iter().find(|c| c.id == order_id).cloned();

    let user_vk = sk.read().verifying_key();

    // Form signals
    let mut display_name = use_signal(String::new);
    let mut order_text = use_signal(String::new);
    let mut price_input = use_signal(String::new);
    let mut price_error = use_signal(|| Option::<String>::None);
    let mut show_add_form = use_signal(|| false);
    let mut edit_mode = use_signal(|| false);
    let mut show_invite_copied = use_signal(|| false);

    let user_item = contract.as_ref().and_then(|c| {
        let vk = user_vk;
        c.state.items.items.iter().find_map(|ai| {
            if ai.item.signed_by == vk {
                match &ai.item.content {
                    ItemContentV1::Item { display_name, order, price_cents } => {
                        Some((display_name.clone(), order.clone(), *price_cents))
                    }
                    _ => None,
                }
            } else {
                None
            }
        })
    });

    let total_cents = contract.as_ref().map(|c| {
        c.state.items.items.iter().filter_map(|ai| {
            match &ai.item.content {
                ItemContentV1::Item { price_cents, .. } => Some(*price_cents),
                _ => None,
            }
        }).sum::<u64>()
    }).unwrap_or(0);

    let paid_cents = contract.as_ref().map(|c| {
        c.state.items.items.iter().filter_map(|ai| {
            match &ai.item.content {
                ItemContentV1::Item { price_cents, .. } => {
                    let paid = c.state.paid.paid.values.get(&ai.item.signed_by).copied().unwrap_or(false);
                    if paid { Some(*price_cents) } else { None }
                }
                _ => None,
            }
        }).sum::<u64>()
    }).unwrap_or(0);

    let items = contract.as_ref().map(|c| {
        c.state.items.items.iter().filter_map(|ai| {
            match &ai.item.content {
                ItemContentV1::Item { display_name, order, price_cents } => {
                    Some((ai.item.signed_by, display_name.clone(), order.clone(), *price_cents))
                }
                _ => None,
            }
        }).collect::<Vec<_>>()
    }).unwrap_or_default();

    let mut handle_add_item = {
        let order_id = order_id.clone();
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
            let user_key = sk.read().clone();
            let user_vk_val = user_key.verifying_key();

            let current_order_id = order_id.clone();
            contracts.with_mut(|all_contracts| {
                if let Some(c) = all_contracts.iter_mut().find(|c| c.id == current_order_id) {
                    let next_version = c.state.items.items.iter()
                        .find(|it| it.item.signed_by == user_vk_val)
                        .map(|it| it.item.version + 1)
                        .unwrap_or(1);

                    let item = ItemV1 {
                        signed_by: user_vk_val,
                        owner_sign: false,
                        version: next_version,
                        content: ItemContentV1::Item {
                            display_name: dn,
                            order: ot,
                            price_cents: price,
                        },
                    };
                    let current_state = c.state.clone();
                    let _ = c.state.apply_delta(&current_state, &c.parameters, &Some(FullOrderStateV1Delta {
                        order: None,
                        items: Some(vec![AuthorizedItemV1::new(item, &user_key)]),
                        paid: None,
                        version: None,
                    }));
                }
            });

            display_name.set(String::new());
            order_text.set(String::new());
            price_input.set(String::new());
            show_add_form.set(false);
        }
    };

    let mut handle_edit_item = {
        let order_id = order_id.clone();
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
            let user_key = sk.read().clone();
            let user_vk_val = user_key.verifying_key();

            let current_order_id = order_id.clone();
            contracts.with_mut(|all_contracts| {
                if let Some(c) = all_contracts.iter_mut().find(|c| c.id == current_order_id) {
                    if let Some(pos) = c.state.items.items.iter().position(|it| it.item.signed_by == user_vk_val) {
                        let existing = &c.state.items.items[pos];
                        
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
                        c.state.items.items[pos] = AuthorizedItemV1::new(new_item, &user_key);
                    }
                }
            });
            edit_mode.set(false);
        }
    };

    let mut handle_delete_item = {
        let order_id = order_id.clone();
        move || {
            let user_vk_val = user_vk;
            let owner_sk = sk.read().clone();
            let current_order_id = order_id.clone();
            contracts.with_mut(|all_contracts| {
                if let Some(c) = all_contracts.iter_mut().find(|c| c.id == current_order_id) {
                    if let Some(item) = c.state.items.items.iter().find(|it| it.item.signed_by == user_vk_val) {
                        let new_item = ItemV1 {
                            signed_by: item.item.signed_by,
                            owner_sign: false,
                            content: ItemContentV1::Deleted {},
                            version: item.item.version + 1,
                        };
                        let current_state = c.state.clone();
                        let _ = c.state.apply_delta(&current_state, &c.parameters, &Some(FullOrderStateV1Delta {
                            order: None,
                            items: Some(vec![AuthorizedItemV1::new(new_item, &owner_sk)]),
                            paid: None,
                            version: None,
                        }));
                    }
                }
            });
        }
    };

    let handle_update_paid = {
        let order_id = order_id.clone();
        move |target_user: VerifyingKey, is_paid: bool| {
            let owner_sk = sk.read().clone();
            let current_order_id = order_id.clone();
            contracts.with_mut(|all_contracts| {
                if let Some(c) = all_contracts.iter_mut().find(|c| c.id == current_order_id) {
                    // Verify caller is owner
                    if owner_sk.verifying_key() != c.parameters.owner {
                        return;
                    }

                    let mut paid_map = c.state.paid.paid.values.clone();
                    paid_map.insert(target_user, is_paid);
                    let new_paid = Paid {
                        values: paid_map,
                        paid_version: c.state.paid.paid.paid_version + 1,
                    };
                    let delta: FullOrderStateV1Delta = FullOrderStateV1Delta {
                        order: None,
                        items: None,
                        paid: Some(AuthorizedPaidV1::new(new_paid, &owner_sk)),
                        version: None,
                    };
                    let current_state = c.state.clone();
                    let _ = c.state.apply_delta(&current_state, &c.parameters, &Some(delta));
                }
            });
        }
    };
    let mut handle_update_paid = use_signal(|| handle_update_paid);

    rsx! {
        match contract {
            None => rsx! {
                div {
                    class: "empty-state",
                    h3 { "Order not found" }
                }
            },
            Some(ref c) => {
                let order_name = c.state.order.order.name.clone();
                let created_at = c.parameters.created_at.to_rfc3339();
                let is_creator = c.parameters.owner == user_vk;
                let items_count = items.len();
                let total_cents_val = total_cents;
                let paid_cents_val = paid_cents;

                rsx! {
                    div {
                        class: "content-header",
                        div {
                            h2 { "{order_name}" }
                            div {
                                class: "header-meta",
                                "Created {created_at}"
                                if is_creator {
                                    span {
                                        style: "color: var(--primary-color)",
                                        " Admin"
                                    }
                                }
                            }
                        }
                        div {
                            class: "header-actions",
                            button {
                                class: "btn btn-secondary",
                                onclick: move |_| {
                                    show_invite_copied.set(true);
                                },
                                if show_invite_copied() { "Invite link copied!" } else { "Invite other users (will copy link)" }
                            }
                        }
                    }

                    div {
                        class: "content-body",
                        div {
                            class: "summary-card",
                            div {
                                class: "summary-row",
                                span { "Total Items" }
                                span { "{items_count}" }
                            }
                            div {
                                class: "summary-row",
                                span { "Paid" }
                                span { "{format_price(paid_cents_val)} / {format_price(total_cents_val)}" }
                            }
                            div {
                                class: "summary-row total",
                                span { "Outstanding" }
                                span { "{format_price(total_cents_val - paid_cents_val)}" }
                            }
                        }

                        div {
                            class: "your-order-section",
                            h4 { "Your Order" }
                            match user_item {
                                Some(item) => {
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
                                                            value: "{item.0}",
                                                            oninput: move |e| display_name.set(e.value())
                                                        }
                                                    }
                                                    div {
                                                        class: "form-group",
                                                        label { "Price" }
                                                        input {
                                                            r#type: "text",
                                                            value: "{format_price(item.2)}",
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
                                                        oninput: move |e| order_text.set(e.value()),
                                                        "{item.1}"
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
                                        let is_paid = c.state.paid.paid.values.get(&user_vk).copied().unwrap_or(false);
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
                                }
                                None => {
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

                        h3 {
                            style: "margin: 20px 0 15px;",
                            "All Orders"
                        }
                        if items.is_empty() {
                            div {
                                style: "text-align: center; padding: 40px; color: var(--text-muted);",
                                "No orders yet. Be the first to add one!"
                            }
                        } else {
                            div {
                                class: "items-table",
                                table {
                                    thead {
                                        tr {
                                            th { "Name" }
                                            th { "Order" }
                                            th { "Price" }
                                            th {
                                                class: "checkbox-cell",
                                                "Paid"
                                            }
                                        }
                                    }
                                    tbody {
                                        for it in items.iter() {
                                            {
                                                let user_key = it.0;
                                                let dn = it.1.clone();
                                                let ord = it.2.clone();
                                                let price_cents = it.3;
                                                let is_own = user_key == user_vk;
                                                let item_paid = contract.as_ref().and_then(|c| {
                                                    c.state.paid.paid.values.get(&user_key).copied()
                                                }).unwrap_or(false);

                                                rsx! {
                                                    tr {
                                                        td {
                                                            "{dn}"
                                                            if is_own {
                                                                span {
                                                                    style: "margin-left: 8px; font-size: 0.8em; color: var(--secondary-color);",
                                                                    "(you)"
                                                                }
                                                            }
                                                        }
                                                        td { "{ord}" }
                                                        td {
                                                            class: "price-cell",
                                                            "{format_price(price_cents)}"
                                                        }
                                                        td {
                                                            class: "checkbox-cell",
                                                            input {
                                                                class: "paid-checkbox",
                                                                r#type: "checkbox",
                                                                checked: item_paid,
                                                                disabled: !is_creator,
                                                                onchange: move |e| {
                                                                    let checked = e.checked();
                                                                    handle_update_paid.write()(user_key, checked);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn format_price(cents: u64) -> String {
    let dollars = cents / 100;
    let cents_part = cents % 100;
    format!("${}.{:02}", dollars, cents_part)
}

fn parse_price(input: &str) -> Result<u64, String> {
    let clean: String = input.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',').collect();
    if clean.is_empty() {
        return Err("Price cannot be empty".to_string());
    }

    let split_at = if clean.contains(',') { ',' } else { '.' };

    if let Some((dollars, cents)) = clean.split_once(split_at) {
        let d: u64 = if dollars.is_empty() { 0 } else { dollars.parse().map_err(|_| "Invalid dollar amount".to_string())? };
        let mut c_str = cents.to_string();
        c_str.push_str("00");
        let c: u64 = c_str[..2].parse().map_err(|_| "Invalid cents amount".to_string())?;
        Ok(d * 100 + c)
    } else {
        let d: u64 = clean.parse().map_err(|_| "Invalid price format".to_string())?;
        Ok(d * 100)
    }
}
