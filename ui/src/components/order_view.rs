//! Order View Component
//!
//! Displays the details of a pizza order including all items in a table.

use crate::services::{AppState, UserIdentity};
use dioxus::prelude::*;

#[component]
pub fn OrderView(
    order_id: String,
    mut app_state: Signal<AppState>,
    identity: Signal<UserIdentity>,
) -> Element {
    // Form state for adding new item
    let mut display_name = use_signal(|| String::new());
    let mut order_text = use_signal(|| String::new());
    let mut price_input = use_signal(|| String::new());
    let mut show_add_form = use_signal(|| false);
    let mut edit_mode = use_signal(|| false);

    let state = app_state.read();
    let order = match state.get_order(&order_id) {
        Some(o) => o,
        None => {
            return rsx! {
                div { class: "empty-state",
                    h3 { "Order not found" }
                }
            }
        }
    };

    let user_id = identity.read().user_id();
    let is_creator = order.params.creator == identity.read().verifying_key().to_bytes();

    // Get user's existing item if any
    let user_item = order.state.items.items.get(&user_id).cloned();

    // Calculate totals
    let total_cents: u64 = order.state.items.items.values().map(|i| i.price_cents).sum();
    let paid_cents: u64 = order
        .state
        .items
        .items
        .values()
        .filter(|i| i.paid)
        .map(|i| i.price_cents)
        .sum();

    let order_name = order.state.config.name.clone();
    let created_at = order
        .state
        .config
        .created_at
        .map(|dt| dt.format("%B %d, %Y at %H:%M").to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // Clone items for iteration
    let items: Vec<_> = order
        .state
        .items
        .items
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let items_count = items.len();

    // Clone for closures
    let order_id_for_add = order_id.clone();
    let order_id_for_edit = order_id.clone();
    let order_id_for_delete = order_id.clone();

    rsx! {
        div { class: "content-header",
            div {
                h2 { "{order_name}" }
                div { class: "header-meta",
                    "Created {created_at}"
                    if is_creator {
                        span { " · You are the creator" }
                    }
                }
            }
        }

        div { class: "content-body",
            // Summary card
            div { class: "summary-card",
                div { class: "summary-row",
                    span { "Total Items" }
                    span { "{items_count}" }
                }
                div { class: "summary-row",
                    span { "Paid" }
                    span { "{format_price(paid_cents)} / {format_price(total_cents)}" }
                }
                div { class: "summary-row total",
                    span { "Outstanding" }
                    span { "{format_price(total_cents - paid_cents)}" }
                }
            }

            // User's order section
            div { class: "your-order-section",
                h4 { "Your Order" }

                if let Some(ref item) = user_item {
                    if *edit_mode.read() {
                        // Edit form
                        form {
                            onsubmit: {
                                let order_id = order_id_for_edit.clone();
                                move |e: Event<FormData>| {
                                    e.prevent_default();
                                    let dn = display_name.read().clone();
                                    let ot = order_text.read().clone();
                                    let pi = price_input.read().clone();

                                    let price = parse_price(&pi);
                                    let dn_opt = if dn.is_empty() { None } else { Some(dn) };
                                    let ot_opt = if ot.is_empty() { None } else { Some(ot) };

                                    let _ = app_state.write().update_item(
                                        &order_id,
                                        dn_opt,
                                        ot_opt,
                                        Some(price),
                                        identity.read().signing_key(),
                                    );
                                    edit_mode.set(false);
                                }
                            },
                            div { class: "form-row",
                                div { class: "form-group",
                                    label { "Display Name" }
                                    input {
                                        r#type: "text",
                                        value: "{item.display_name}",
                                        oninput: move |e| display_name.set(e.value()),
                                    }
                                }
                                div { class: "form-group",
                                    label { "Price" }
                                    input {
                                        r#type: "text",
                                        value: "{format_price(item.price_cents)}",
                                        oninput: move |e| price_input.set(e.value()),
                                    }
                                }
                            }
                            div { class: "form-group",
                                label { "Order" }
                                textarea {
                                    value: "{item.order}",
                                    oninput: move |e| order_text.set(e.value()),
                                }
                            }
                            div { style: "display: flex; gap: 10px;",
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
                    } else {
                        // Display current order
                        div { style: "display: flex; justify-content: space-between; align-items: start;",
                            div {
                                p { strong { "{item.display_name}" } " - {item.order}" }
                                p { style: "color: var(--text-muted);",
                                    "Price: {format_price(item.price_cents)}"
                                    if item.paid {
                                        span { class: "status-badge paid", style: "margin-left: 10px;",
                                            "Paid"
                                        }
                                    }
                                }
                            }
                            div { class: "item-actions",
                                button {
                                    class: "btn btn-small btn-outline",
                                    onclick: move |_| edit_mode.set(true),
                                    "Edit"
                                }
                                button {
                                    class: "btn btn-small btn-outline",
                                    onclick: {
                                        let order_id = order_id_for_delete.clone();
                                        move |_| {
                                            let _ = app_state.write().delete_item(&order_id, identity.read().signing_key());
                                        }
                                    },
                                    "Remove"
                                }
                            }
                        }
                    }
                } else if *show_add_form.read() {
                    // Add new item form
                    form {
                        onsubmit: {
                            let order_id = order_id_for_add.clone();
                            move |e: Event<FormData>| {
                                e.prevent_default();
                                let dn = display_name.read().clone();
                                let ot = order_text.read().clone();
                                let pi = price_input.read().clone();

                                if dn.is_empty() || ot.is_empty() {
                                    return;
                                }

                                let price = parse_price(&pi);

                                let _ = app_state.write().add_item(
                                    &order_id,
                                    dn,
                                    ot,
                                    price,
                                    identity.read().signing_key(),
                                );

                                display_name.set(String::new());
                                order_text.set(String::new());
                                price_input.set(String::new());
                                show_add_form.set(false);
                            }
                        },
                        div { class: "form-row",
                            div { class: "form-group",
                                label { "Your Name" }
                                input {
                                    r#type: "text",
                                    placeholder: "e.g., John",
                                    required: true,
                                    value: "{display_name}",
                                    oninput: move |e| display_name.set(e.value()),
                                }
                            }
                            div { class: "form-group",
                                label { "Price" }
                                input {
                                    r#type: "text",
                                    placeholder: "e.g., 12.50",
                                    value: "{price_input}",
                                    oninput: move |e| price_input.set(e.value()),
                                }
                            }
                        }
                        div { class: "form-group",
                            label { "What would you like?" }
                            textarea {
                                placeholder: "e.g., 1x Margherita, extra cheese",
                                required: true,
                                value: "{order_text}",
                                oninput: move |e| order_text.set(e.value()),
                            }
                        }
                        div { style: "display: flex; gap: 10px;",
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
                } else {
                    button {
                        class: "btn btn-secondary",
                        onclick: move |_| show_add_form.set(true),
                        "+ Add Your Order"
                    }
                }
            }

            // All items table
            h3 { style: "margin: 20px 0 15px;", "All Orders" }

            if items.is_empty() {
                div {
                    style: "text-align: center; padding: 40px; color: var(--text-muted);",
                    "No orders yet. Be the first to add one!"
                }
            } else {
                div { class: "items-table",
                    table {
                        thead {
                            tr {
                                th { "Name" }
                                th { "Order" }
                                th { "Price" }
                                th { class: "checkbox-cell", "Paid" }
                            }
                        }
                        tbody {
                            for (user_key, item) in items.iter() {
                                {
                                    let user_key_clone = user_key.clone();
                                    let is_own = *user_key == user_id;
                                    let item_paid = item.paid;
                                    let order_id_for_paid = order_id.clone();

                                    rsx! {
                                        tr {
                                            key: "{item.display_name}-{item.version}",
                                            td {
                                                "{item.display_name}"
                                                if is_own {
                                                    span {
                                                        style: "margin-left: 8px; font-size: 0.8em; color: var(--secondary-color);",
                                                        "(you)"
                                                    }
                                                }
                                            }
                                            td { "{item.order}" }
                                            td { class: "price-cell", "{format_price(item.price_cents)}" }
                                            td { class: "checkbox-cell",
                                                input {
                                                    class: "paid-checkbox",
                                                    r#type: "checkbox",
                                                    checked: item_paid,
                                                    disabled: !is_creator,
                                                    onchange: {
                                                        let user_key = user_key_clone.clone();
                                                        let order_id = order_id_for_paid.clone();
                                                        move |_| {
                                                            let _ = app_state.write().update_paid(
                                                                &order_id,
                                                                &user_key,
                                                                !item_paid,
                                                                identity.read().signing_key(),
                                                            );
                                                        }
                                                    },
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

fn parse_price(input: &str) -> u64 {
    // Remove $ sign if present
    let clean = input.trim().trim_start_matches('$');

    // Try to parse as decimal
    if let Some((dollars, cents)) = clean.split_once('.') {
        let d: u64 = dollars.parse().unwrap_or(0);
        let c: u64 = cents.get(..2).unwrap_or(cents).parse().unwrap_or(0);
        d * 100 + c
    } else {
        // Whole dollars
        clean.parse::<u64>().unwrap_or(0) * 100
    }
}
