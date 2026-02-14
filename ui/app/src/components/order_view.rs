use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use crate::Contract;
use ed25519_dalek::{SigningKey, VerifyingKey};
use pizza_common::{ComposableState, FullOrderStateV1Delta, ItemsV1};
use pizza_common::order_state::{ItemContentV1, ItemV1, AuthorizedItemV1, Paid, AuthorizedPaidV1};

#[component]
pub fn OrderView(
    contracts: RwSignal<Vec<Contract>>,
    sk: RwSignal<SigningKey>,
) -> impl IntoView {
    let params = use_params_map();
    let order_id = move || params.get().get("id").unwrap_or_default();

    let contract = move || {
        let id = order_id();
        contracts.get().into_iter().find(|c| c.id == id)
    };

    let user_vk = move || sk.get().verifying_key();

    // Form signals
    let (display_name, set_display_name) = signal(String::new());
    let (order_text, set_order_text) = signal(String::new());
    let (price_input, set_price_input) = signal(String::new());
    let (show_add_form, set_show_add_form) = signal(false);
    let (edit_mode, set_edit_mode) = signal(false);
    let (show_invite_copied, set_show_invite_copied) = signal(false);

    let user_item = move || {
        contract().and_then(|c| {
            let vk = user_vk();
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
        })
    };

    let total_cents = move || {
        contract().map(|c| {
            c.state.items.items.iter().filter_map(|ai| {
                match &ai.item.content {
                    ItemContentV1::Item { price_cents, .. } => Some(*price_cents),
                    _ => None,
                }
            }).sum::<u64>()
        }).unwrap_or(0)
    };

    let paid_cents = move || {
        contract().map(|c| {
            c.state.items.items.iter().filter_map(|ai| {
                match &ai.item.content {
                    ItemContentV1::Item { price_cents, .. } => {
                        let paid = c.state.paid.paid.values.get(&ai.item.signed_by).copied().unwrap_or(false);
                        if paid { Some(*price_cents) } else { None }
                    }
                    _ => None,
                }
            }).sum::<u64>()
        }).unwrap_or(0)
    };

    let items = move || {
        contract().map(|c| {
            c.state.items.items.iter().filter_map(|ai| {
                match &ai.item.content {
                    ItemContentV1::Item { display_name, order, price_cents } => {
                        Some((ai.item.signed_by, display_name.clone(), order.clone(), *price_cents))
                    }
                    _ => None,
                }
            }).collect::<Vec<_>>()
        }).unwrap_or_default()
    };

    let handle_add_item = move |e: leptos::web_sys::SubmitEvent| {
        e.prevent_default();
        let dn = display_name.get();
        let ot = order_text.get();
        let pi = price_input.get();

        if dn.is_empty() || ot.is_empty() {
            return;
        }

        let price = parse_price(&pi);
        let user_key = sk.get();
        let user_vk_val = user_key.verifying_key();

        contracts.update(|all_contracts| {
            if let Some(c) = all_contracts.iter_mut().find(|c| c.id == order_id()) {
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

        set_display_name.set(String::new());
        set_order_text.set(String::new());
        set_price_input.set(String::new());
        set_show_add_form.set(false);
    };

    let handle_edit_item = move |e: leptos::web_sys::SubmitEvent| {
        e.prevent_default();
        let dn = display_name.get();
        let ot = order_text.get();
        let pi = price_input.get();

        let price = parse_price(&pi);
        let user_key = sk.get();
        let user_vk_val = user_key.verifying_key();

        contracts.update(|all_contracts| {
            if let Some(c) = all_contracts.iter_mut().find(|c| c.id == order_id()) {
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
        set_edit_mode.set(false);
    };

    let handle_delete_item = move || {
        let user_vk_val = user_vk();
        let owner_sk = sk.get();
        contracts.update(|all_contracts| {
            if let Some(c) = all_contracts.iter_mut().find(|c| c.id == order_id()) {
                if let Some(item) = c.state.items.items.iter().find(|it| it.item.signed_by == user_vk_val) {
                    // c.state.items.items.remove(pos);
                    let new_item = ItemV1 {
                        signed_by: item.item.signed_by,
                        owner_sign: false,
                        content: ItemContentV1::Deleted {},
                        version: item.item.version + 1,
                    };
                    let current_state = c.state.clone();
                    let _ = c.state.apply_delta(&current_state, &c.parameters, &Some(FullOrderStateV1Delta {
                        order: None,
                        items: Some(vec![AuthorizedItemV1::new(new_item, &owner_sk)].into()),
                        paid: None,
                        version: None,
                    }));
                }
            }
        });
    };

    let handle_update_paid = move |target_user: VerifyingKey, is_paid: bool| {
        let owner_sk = sk.get();
        contracts.update(|all_contracts| {
            if let Some(c) = all_contracts.iter_mut().find(|c| c.id == order_id()) {
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
    };

    view! {
        {move || match contract() {
            None => view! {
                <div class="empty-state">
                    <h3>"Order not found"</h3>
                </div>
            }.into_any(),
            Some(c) => {
                let order_name = c.state.order.order.name.clone();
                let created_at = c.parameters.created_at.to_rfc3339();
                let is_creator = c.parameters.owner == user_vk();
                let items_count = items().len();
                let total_cents_val = total_cents();
                let paid_cents_val = paid_cents();

                view! {
                    <div class="content-header">
                        <div>
                            <h2>{order_name}</h2>
                            <div class="header-meta">
                                "Created " {created_at}
                                {if is_creator {
                                    view! { <span style="color: var(--primary-color)">" Admin"</span> }.into_any()
                                } else {
                                    view! {}.into_any()
                                }}
                            </div>
                        </div>
                        <div class="header-actions">
                            <button class="btn btn-secondary" on:click=move |_| {
                                // Simple placeholder for invite
                                set_show_invite_copied.set(true);
                                // In real app would copy to clipboard
                            }>
                                {move || if show_invite_copied.get() { "Invite link copied!" } else { "Invite other users (will copy link)" }}
                            </button>
                        </div>
                    </div>

                    <div class="content-body">
                        <div class="summary-card">
                            <div class="summary-row">
                                <span>"Total Items"</span>
                                <span>{items_count}</span>
                            </div>
                            <div class="summary-row">
                                <span>"Paid"</span>
                                <span>{format_price(paid_cents_val)} " / " {format_price(total_cents_val)}</span>
                            </div>
                            <div class="summary-row total">
                                <span>"Outstanding"</span>
                                <span>{format_price(total_cents_val - paid_cents_val)}</span>
                            </div>
                        </div>

                        <div class="your-order-section">
                            <h4>"Your Order"</h4>
                            {move || match user_item() {
                                Some(item) => {
                                    let item_view = item.clone();
                                    if edit_mode.get() {
                                        view! {
                                            <form on:submit=handle_edit_item>
                                                <div class="form-row">
                                                    <div class="form-group">
                                                        <label>"Display Name"</label>
                                                        <input
                                                            type="text"
                                                            prop:value=item_view.0
                                                            on:input=move |e| set_display_name.set(event_target_value(&e))
                                                        />
                                                    </div>
                                                    <div class="form-group">
                                                        <label>"Price"</label>
                                                        <input
                                                            type="text"
                                                            prop:value=format_price(item_view.2)
                                                            on:input=move |e| set_price_input.set(event_target_value(&e))
                                                        />
                                                    </div>
                                                </div>
                                                <div class="form-group">
                                                    <label>"Order"</label>
                                                    <textarea
                                                        on:input=move |e| set_order_text.set(event_target_value(&e))
                                                    >{item_view.1}</textarea>
                                                </div>
                                                <div style="display: flex; gap: 10px;">
                                                    <button class="btn btn-primary" type="submit">"Save Changes"</button>
                                                    <button class="btn btn-outline" type="button" on:click=move |_| set_edit_mode.set(false)>"Cancel"</button>
                                                </div>
                                            </form>
                                        }.into_any()
                                    } else {
                                        let c = contract().unwrap();
                                        let is_paid = c.state.paid.paid.values.get(&user_vk()).copied().unwrap_or(false);
                                        let item_view_2 = item_view.clone();
                                        view! {
                                            <div style="display: flex; justify-content: space-between; align-items: start;">
                                                <div>
                                                    <p><strong>{item_view.0}</strong> " - " {item_view.1}</p>
                                                    <p style="color: var(--text-muted);">
                                                        "Price: " {format_price(item_view.2)}
                                                        {if is_paid {
                                                            view! { <span class="status-badge paid" style="margin-left: 10px;">"Paid"</span> }.into_any()
                                                        } else {
                                                            view! {}.into_any()
                                                        }}
                                                    </p>
                                                </div>
                                                <div class="item-actions">
                                                    <button class="btn btn-small btn-outline" on:click=move |_| {
                                                        set_display_name.set(item_view_2.0.clone());
                                                        set_order_text.set(item_view_2.1.clone());
                                                        set_price_input.set(format_price(item_view_2.2));
                                                        set_edit_mode.set(true);
                                                    }>"Edit"</button>
                                                    <button class="btn btn-small btn-outline" on:click=move |_| handle_delete_item()>"Remove"</button>
                                                </div>
                                            </div>
                                        }.into_any()
                                    }
                                }
                                None => {
                                    if show_add_form.get() {
                                        view! {
                                            <form on:submit=handle_add_item>
                                                <div class="form-row">
                                                    <div class="form-group">
                                                        <label>"Your Name"</label>
                                                        <input
                                                            type="text"
                                                            placeholder="e.g., John"
                                                            required=true
                                                            on:input=move |e| set_display_name.set(event_target_value(&e))
                                                        />
                                                    </div>
                                                    <div class="form-group">
                                                        <label>"Price"</label>
                                                        <input
                                                            type="text"
                                                            placeholder="e.g., 12.50"
                                                            on:input=move |e| set_price_input.set(event_target_value(&e))
                                                        />
                                                    </div>
                                                </div>
                                                <div class="form-group">
                                                    <label>"What would you like?"</label>
                                                    <textarea
                                                        placeholder="e.g., 1x Margherita, extra cheese"
                                                        required=true
                                                        on:input=move |e| set_order_text.set(event_target_value(&e))
                                                    ></textarea>
                                                </div>
                                                <div style="display: flex; gap: 10px;">
                                                    <button class="btn btn-primary" type="submit">"Add My Order"</button>
                                                    <button class="btn btn-outline" type="button" on:click=move |_| set_show_add_form.set(false)>"Cancel"</button>
                                                </div>
                                            </form>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <button class="btn btn-secondary" on:click=move |_| set_show_add_form.set(true)>
                                                "+ Add Your Order"
                                            </button>
                                        }.into_any()
                                    }
                                }
                            }}
                        </div>

                        <h3 style="margin: 20px 0 15px;">"All Orders"</h3>
                        {move || if items().is_empty() {
                            view! {
                                <div style="text-align: center; padding: 40px; color: var(--text-muted);">
                                    "No orders yet. Be the first to add one!"
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="items-table">
                                    <table>
                                        <thead>
                                            <tr>
                                                <th>"Name"</th>
                                                <th>"Order"</th>
                                                <th>"Price"</th>
                                                <th class="checkbox-cell">"Paid"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            <For
                                                each=items
                                                key=|it| format!("{}-{}", it.1, it.3)
                                                children=move |(user_key, dn, ord, price_cents)| {
                                                    let is_own = user_key == user_vk();
                                                    let item_paid = move || {
                                                        contract().and_then(|c| {
                                                            c.state.paid.paid.values.get(&user_key).copied()
                                                        }).unwrap_or(false)
                                                    };

                                                    view! {
                                                        <tr>
                                                            <td>
                                                                {dn}
                                                                {if is_own {
                                                                    view! { <span style="margin-left: 8px; font-size: 0.8em; color: var(--secondary-color);">"(you)"</span> }.into_any()
                                                                } else {
                                                                    view! {}.into_any()
                                                                }}
                                                            </td>
                                                            <td>{ord}</td>
                                                            <td class="price-cell">{format_price(price_cents)}</td>
                                                            <td class="checkbox-cell">
                                                                <input
                                                                    class="paid-checkbox"
                                                                    type="checkbox"
                                                                    prop:checked=item_paid
                                                                    disabled=!is_creator
                                                                    on:change=move |e| {
                                                                        let checked = event_target_checked(&e);
                                                                        handle_update_paid(user_key, checked);
                                                                    }
                                                                />
                                                            </td>
                                                        </tr>
                                                    }
                                                }
                                            />
                                        </tbody>
                                    </table>
                                </div>
                            }.into_any()
                        }}
                    </div>
                }.into_any()
            }
        }}
    }
}

fn format_price(cents: u64) -> String {
    let dollars = cents / 100;
    let cents_part = cents % 100;
    format!("${}.{:02}", dollars, cents_part)
}

fn parse_price(input: &str) -> u64 {
    let clean = input.trim().trim_start_matches('$');
    if let Some((dollars, cents)) = clean.split_once('.') {
        let d: u64 = dollars.parse().unwrap_or(0);
        let mut c_str = cents.to_string();
        c_str.push_str("00");
        let c: u64 = c_str[..2].parse().unwrap_or(0);
        d * 100 + c
    } else {
        clean.parse::<u64>().unwrap_or(0) * 100
    }
}
