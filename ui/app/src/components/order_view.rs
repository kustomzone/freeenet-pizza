use dioxus::prelude::*;
use crate::services::{BaseService, Contract};
use crate::components::YourOrderSection;
use ed25519_dalek::VerifyingKey;
use pizza_common::FullOrderStateV1Delta;
use pizza_common::order_state::{ItemContentV1, Paid, AuthorizedPaidV1};
use pizza_common::util::format_price;
use futures::StreamExt;

#[derive(Clone, PartialEq)]
enum LoadState {
    Loading,
    Loaded(Contract),
    NotFound,
}

#[component]
pub fn OrderViewComponent(
    id: String,
) -> Element {
    let base = use_context::<BaseService>();
    let sk = base.get_private_key().unwrap();
    let user_vk = base.get_public_key().unwrap();

    // Start with cached state if available, otherwise loading
    let initial_state = match base.get_contract_cached(id.clone()) {
        Some(c) => LoadState::Loaded(c),
        None => LoadState::Loading,
    };
    let mut load_state = use_signal(|| initial_state);

    // Fetch contract asynchronously and subscribe to updates
    use_effect({
        let id = id.clone();
        let base = base.clone();
        move || {
            let id = id.clone();
            let base = base.clone();
            spawn(async move {
                // Fetch from network
                match base.get_contract_async(id.clone()).await {
                    Ok(contract) => {
                        load_state.set(LoadState::Loaded(contract));
                    }
                    Err(_) => {
                        // Only set NotFound if we don't have cached data
                        if matches!(*load_state.read(), LoadState::Loading) {
                            load_state.set(LoadState::NotFound);
                        }
                    }
                }

                // Subscribe to updates
                let mut stream = base.subscribe_contract_state(id);
                while let Some(new_contract) = stream.next().await {
                    load_state.set(LoadState::Loaded(new_contract));
                }
            });
        }
    });

    let mut show_invite_copied = use_signal(|| false);

    let state = load_state.read();
    match &*state {
        LoadState::Loading => rsx! {
            div {
                class: "empty-state loading-state",
                div {
                    class: "loading-spinner",
                }
                h3 { "Loading order..." }
                p { "Fetching contract from Freenet" }
            }
        },
        LoadState::NotFound => rsx! {
            div {
                class: "empty-state",
                h3 { "Order not found" }
                p { "This order may have been deleted or doesn't exist." }
            }
        },
        LoadState::Loaded(c) => {
            let order_name = c.state.order.order.name.clone();
            let created_at = c.parameters.created_at.to_rfc3339();
            let is_creator = c.parameters.owner == user_vk;

            let total_cents = c.state.items.items.iter().filter_map(|ai| {
                match &ai.item.content {
                    ItemContentV1::Item { price_cents, .. } => Some(*price_cents),
                    _ => None,
                }
            }).sum::<u64>();

            let paid_cents = c.state.items.items.iter().filter_map(|ai| {
                match &ai.item.content {
                    ItemContentV1::Item { price_cents, .. } => {
                        let paid = c.state.paid.paid.values.get(&ai.item.signed_by).copied().unwrap_or(false);
                        if paid { Some(*price_cents) } else { None }
                    }
                    _ => None,
                }
            }).sum::<u64>();

            let items_vec = c.state.items.items.iter().filter_map(|ai| {
                match &ai.item.content {
                    ItemContentV1::Item { display_name, order, price_cents } => {
                        Some((ai.item.signed_by, display_name.clone(), order.clone(), *price_cents))
                    }
                    _ => None,
                }
            }).collect::<Vec<_>>();

            let handle_update_paid = {
                let id = id.clone();
                let base = use_context::<BaseService>();
                let sk = sk.clone();
                let c = c.clone();
                move |target_user: VerifyingKey, is_paid: bool| {
                    let owner_sk = sk.clone();
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
                    let _ = base.publish_delta(id.clone(), delta.clone());
                }
            };
            let handle_update_paid = use_signal(move || handle_update_paid);

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
                                if let Some(window) = web_sys::window() {
                                    if let Ok(href) = window.location().href() {
                                        let clipboard = window.navigator().clipboard();
                                        let _ = clipboard.write_text(&href);
                                        show_invite_copied.set(true);
                                    }
                                }
                            },
                            if show_invite_copied() { "Invite link copied!" } else { "Invite" }
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
                            span { "{items_vec.len()}" }
                        }
                        div {
                            class: "summary-row",
                            span { "Paid" }
                            span { "{format_price(paid_cents)} / {format_price(total_cents)}" }
                        }
                        div {
                            class: "summary-row total",
                            span { "Outstanding" }
                            span { "{format_price(total_cents - paid_cents)}" }
                        }
                    }

                    YourOrderSection {
                        id: id.clone(),
                        contract: c.clone(),
                        user_vk: user_vk,
                        sk: sk.clone()
                    }

                    h3 {
                        style: "margin: 20px 0 15px;",
                        "All Orders"
                    }
                    {
                        if items_vec.is_empty() {
                            rsx! {
                                div {
                                    style: "text-align: center; padding: 40px; color: var(--text-muted);",
                                    "No orders yet. Be the first to add one!"
                                }
                            }
                        } else {
                            rsx! {
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
                                            for it in items_vec.iter() {
                                                {
                                                    let user_key = it.0;
                                                    let dn = it.1.clone();
                                                    let ord = it.2.clone();
                                                    let price_cents = it.3;
                                                    let is_own = user_key == user_vk;
                                                    let item_paid = c.state.paid.paid.values.get(&user_key).copied().unwrap_or(false);

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
                                                                        handle_update_paid.read()(user_key, checked);
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
}
