use dioxus::prelude::*;
use crate::services::{BaseService, Contract};
use crate::components::{YourOrderSection, AdminOrderModal, AdminOrderMode, OrderSettingsForm, OrderSettings};
use ed25519_dalek::VerifyingKey;
use pizza_common::FullOrderStateV1Delta;
use pizza_common::order_state::{ItemContentV1, ItemV1, AuthorizedItemV1, Paid, AuthorizedPaidV1, Order, AuthorizedOrderV1};
use pizza_common::util::format_price_with_currency;
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

    // Track the current id to detect changes
    let mut current_id = use_signal(|| id.clone());
    let mut load_state = use_signal(|| LoadState::Loading);

    // Reset state when id changes
    if *current_id.read() != id {
        current_id.set(id.clone());
        // Set initial state from cache or loading
        match base.get_contract_cached(id.clone()) {
            Some(c) => load_state.set(LoadState::Loaded(c)),
            None => load_state.set(LoadState::Loading),
        };
    }

    // Fetch contract and subscribe to updates
    use_effect({
        let id = id.clone();
        let base = base.clone();
        move || {
            let id = id.clone();
            let base = base.clone();
            spawn(async move {
                // Fetch contract (tries cache first, then network)
                match base.get_contract(id.clone()).await {
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

    // Admin modal state - None when closed, Some(mode) when open
    let mut admin_modal_mode: Signal<Option<AdminOrderMode>> = use_signal(|| None);

    // Edit order settings modal state
    let mut show_edit_settings = use_signal(|| false);

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
            let currency = c.state.order.order.currency.clone();
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
                        Some((ai.item.signed_by, display_name.clone(), order.clone(), *price_cents, ai.item.owner_sign))
                    }
                    _ => None,
                }
            }).collect::<Vec<_>>();

            let handle_update_paid = {
                let id = id.clone();
                let base = use_context::<BaseService>();
                let sk = sk.clone();
                let owner_vk = c.parameters.owner;
                move |target_user: VerifyingKey, is_paid: bool| {
                    let owner_sk = sk.clone();
                    // Verify caller is owner
                    if owner_sk.verifying_key() != owner_vk {
                        return;
                    }

                    // Get current state from cache to avoid stale data
                    let current_state = match base.get_contract_cached(id.clone()) {
                        Some(contract) => contract,
                        None => return,
                    };

                    let mut paid_map = current_state.state.paid.paid.values.clone();
                    paid_map.insert(target_user, is_paid);
                    let new_paid = Paid {
                        values: paid_map,
                        paid_version: current_state.state.paid.paid.paid_version + 1,
                    };
                    let delta: FullOrderStateV1Delta = FullOrderStateV1Delta {
                        order: None,
                        items: None,
                        paid: Some(AuthorizedPaidV1::new(new_paid, &owner_sk)),
                        version: None,
                    };
                    let base = base.clone();
                    let id = id.clone();
                    spawn(async move {
                        let _ = base.publish_delta(id, delta).await;
                    });
                }
            };
            let handle_update_paid = use_signal(move || handle_update_paid);

            // Store values needed for admin handlers
            let admin_id = id.clone();
            let admin_base = use_context::<BaseService>();
            let admin_sk = sk.clone();
            let admin_owner_vk = c.parameters.owner;

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
                        if is_creator {
                            button {
                                class: "btn btn-outline",
                                onclick: move |_| show_edit_settings.set(true),
                                "Edit Settings"
                            }
                        }
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
                            span { "{format_price_with_currency(paid_cents, &currency)} / {format_price_with_currency(total_cents, &currency)}" }
                        }
                        div {
                            class: "summary-row total",
                            span { "Outstanding" }
                            span { "{format_price_with_currency(total_cents - paid_cents, &currency)}" }
                        }
                    }

                    YourOrderSection {
                        id: id.clone(),
                        contract: c.clone(),
                        user_vk: user_vk,
                        sk: sk.clone()
                    }

                    div {
                        style: "display: flex; justify-content: space-between; align-items: center; margin: 20px 0 15px;",
                        h3 { "All Orders" }
                        if is_creator {
                            button {
                                class: "btn btn-small btn-secondary",
                                onclick: move |_| {
                                    admin_modal_mode.set(Some(AdminOrderMode::Add));
                                },
                                "+ Add Order"
                            }
                        }
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
                                                if is_creator {
                                                    th { "Actions" }
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
                                                    let owner_sign = it.4;
                                                    let is_own = user_key == user_vk;
                                                    let item_paid = c.state.paid.paid.values.get(&user_key).copied().unwrap_or(false);

                                                    let dn_for_edit = dn.clone();
                                                    let ord_for_edit = ord.clone();

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
                                                                if owner_sign {
                                                                    span {
                                                                        style: "margin-left: 8px; font-size: 0.8em; color: var(--primary-color);",
                                                                        "(admin edited)"
                                                                    }
                                                                }
                                                            }
                                                            td { "{ord}" }
                                                            td {
                                                                class: "price-cell",
                                                                "{format_price_with_currency(price_cents, &currency)}"
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
                                                            if is_creator {
                                                                td {
                                                                    class: "actions-cell",
                                                                    button {
                                                                        class: "btn btn-small btn-outline",
                                                                        onclick: move |_| {
                                                                            admin_modal_mode.set(Some(AdminOrderMode::Edit {
                                                                                target_vk: user_key,
                                                                                initial_name: dn_for_edit.clone(),
                                                                                initial_order: ord_for_edit.clone(),
                                                                                initial_price: price_cents,
                                                                            }));
                                                                        },
                                                                        "Edit"
                                                                    }
                                                                    button {
                                                                        class: "btn btn-small btn-outline",
                                                                        style: "margin-left: 4px;",
                                                                        onclick: {
                                                                            let id = admin_id.clone();
                                                                            let base = admin_base.clone();
                                                                            let owner_sk = admin_sk.clone();
                                                                            let owner_vk = admin_owner_vk;
                                                                            move |_| {
                                                                                // Verify caller is owner
                                                                                if owner_sk.verifying_key() != owner_vk {
                                                                                    return;
                                                                                }

                                                                                // Get current state from cache
                                                                                let current_state = match base.get_contract_cached(id.clone()) {
                                                                                    Some(contract) => contract,
                                                                                    None => return,
                                                                                };

                                                                                // Find the existing item
                                                                                if let Some(existing) = current_state.state.items.items.iter().find(|it| it.item.signed_by == user_key) {
                                                                                    let new_item = ItemV1 {
                                                                                        signed_by: user_key,
                                                                                        owner_sign: true, // Admin is deleting
                                                                                        version: existing.item.version + 1,
                                                                                        content: ItemContentV1::Deleted {},
                                                                                    };
                                                                                    let delta = FullOrderStateV1Delta {
                                                                                        order: None,
                                                                                        items: Some(vec![AuthorizedItemV1::new(new_item, &owner_sk)]),
                                                                                        paid: None,
                                                                                        version: None,
                                                                                    };
                                                                                    let base = base.clone();
                                                                                    let id = id.clone();
                                                                                    spawn(async move {
                                                                                        let _ = base.publish_delta(id, delta).await;
                                                                                    });
                                                                                }
                                                                            }
                                                                        },
                                                                        "Delete"
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

                    // Admin order modal (add or edit)
                    if let Some(mode) = admin_modal_mode.read().clone() {
                        {
                            let id_for_refresh = admin_id.clone();
                            let base_for_refresh = admin_base.clone();
                            rsx! {
                                AdminOrderModal {
                                    contract_id: admin_id.clone(),
                                    owner_sk: admin_sk.clone(),
                                    mode: mode,
                                    currency: currency.clone(),
                                    on_close: move |_| {
                                        admin_modal_mode.set(None);
                                        // Refresh state after modal closes to ensure we have latest data
                                        let base = base_for_refresh.clone();
                                        let id = id_for_refresh.clone();
                                        spawn(async move {
                                            // Small delay to allow publish_delta to complete
                                            gloo_timers::future::TimeoutFuture::new(100).await;
                                            if let Ok(contract) = base.get_contract(id).await {
                                                load_state.set(LoadState::Loaded(contract));
                                            }
                                        });
                                    },
                                }
                            }
                        }
                    }

                    // Edit order settings modal (admin only)
                    if show_edit_settings() {
                        {
                            let id_for_settings = admin_id.clone();
                            let base_for_settings = admin_base.clone();
                            let sk_for_settings = admin_sk.clone();
                            let current_name = order_name.clone();
                            let current_currency = currency.clone();
                            let current_version = c.state.order.order.order_version;
                            rsx! {
                                div {
                                    class: "modal-overlay",
                                    onclick: move |_| show_edit_settings.set(false),
                                    div {
                                        class: "modal",
                                        onclick: |e| e.stop_propagation(),
                                        div {
                                            class: "modal-header",
                                            h3 { "Edit Order Settings" }
                                        }

                                        OrderSettingsForm {
                                            initial_name: current_name,
                                            initial_currency: current_currency,
                                            submit_label: "Save Changes".to_string(),
                                            on_submit: move |settings: OrderSettings| {
                                                let new_order = Order {
                                                    name: settings.name,
                                                    currency: settings.currency,
                                                    order_version: current_version + 1,
                                                };
                                                let authorized_order = AuthorizedOrderV1::new(new_order, &sk_for_settings);
                                                let delta = FullOrderStateV1Delta {
                                                    order: Some(authorized_order),
                                                    items: None,
                                                    paid: None,
                                                    version: None,
                                                };
                                                let base = base_for_settings.clone();
                                                let id = id_for_settings.clone();
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    let _ = base.publish_delta(id, delta).await;
                                                });
                                                show_edit_settings.set(false);
                                            },
                                            on_cancel: move |_| show_edit_settings.set(false),
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
