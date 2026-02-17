use dioxus::prelude::*;
use pizza_common::util::COMMON_CURRENCIES;

/// Data returned when the order settings form is submitted
#[derive(Clone, Debug, PartialEq)]
pub struct OrderSettings {
    pub name: String,
    pub currency: String,
}

/// Reusable form for editing order name and currency.
/// Used in both order creation and admin edit dialogs.
#[component]
pub fn OrderSettingsForm(
    /// Initial order name (empty for new orders)
    #[props(default = String::new())]
    initial_name: String,
    /// Initial currency code (defaults to USD)
    #[props(default = "USD".to_string())]
    initial_currency: String,
    /// Label for the submit button
    submit_label: String,
    /// Called when form is submitted with valid data
    on_submit: EventHandler<OrderSettings>,
    /// Called when cancel is clicked
    on_cancel: EventHandler<()>,
) -> Element {
    let mut order_name = use_signal(|| initial_name.clone());
    let mut currency = use_signal(|| initial_currency.clone());
    let mut currency_search = use_signal(String::new);
    let mut show_currency_dropdown = use_signal(|| false);

    // Filter currencies based on search (by code, case-insensitive)
    let filtered_currencies: Vec<_> = {
        let search = currency_search.read().to_uppercase();
        if search.is_empty() {
            COMMON_CURRENCIES.iter().collect()
        } else {
            COMMON_CURRENCIES
                .iter()
                .filter(|c| c.iso_alpha_code.contains(&search))
                .collect()
        }
    };

    // Get display text for selected currency
    let selected_display = COMMON_CURRENCIES
        .iter()
        .find(|c| c.iso_alpha_code == currency.read().as_str())
        .map(|c| format!("{} {} - {}", c.iso_alpha_code, c.symbol, c.name))
        .unwrap_or_else(|| currency.read().clone());

    rsx! {
        form {
            onsubmit: move |e| {
                e.prevent_default();
                let name = order_name.read().clone();
                let curr = currency.read().clone();
                if !name.is_empty() {
                    on_submit.call(OrderSettings { name, currency: curr });
                }
            },
            div {
                class: "modal-body",
                div {
                    class: "form-group",
                    label { "Order Name" }
                    input {
                        r#type: "text",
                        placeholder: "e.g., Pizza for Friday Party",
                        required: true,
                        autofocus: true,
                        value: "{order_name}",
                        oninput: move |e| order_name.set(e.value())
                    }
                }
                div {
                    class: "form-group",
                    label { "Currency" }
                    div {
                        class: "searchable-select",
                        input {
                            r#type: "text",
                            placeholder: "Search currency code (e.g., USD, EUR)...",
                            value: if show_currency_dropdown() { "{currency_search}" } else { "{selected_display}" },
                            onfocus: move |_| {
                                show_currency_dropdown.set(true);
                                currency_search.set(String::new());
                            },
                            oninput: move |e| {
                                currency_search.set(e.value());
                                show_currency_dropdown.set(true);
                            },
                            onblur: move |_| {
                                // Delay hiding to allow click on option
                                spawn(async move {
                                    gloo_timers::future::TimeoutFuture::new(150).await;
                                    show_currency_dropdown.set(false);
                                });
                            }
                        }
                        if show_currency_dropdown() {
                            div {
                                class: "searchable-select-dropdown",
                                for curr in filtered_currencies.iter() {
                                    div {
                                        class: if curr.iso_alpha_code == currency.read().as_str() { "searchable-select-option selected" } else { "searchable-select-option" },
                                        onmousedown: {
                                            let code = curr.iso_alpha_code.to_string();
                                            move |e| {
                                                e.prevent_default();
                                                currency.set(code.clone());
                                                show_currency_dropdown.set(false);
                                            }
                                        },
                                        "{curr.iso_alpha_code} {curr.symbol} - {curr.name}"
                                    }
                                }
                                if filtered_currencies.is_empty() {
                                    div {
                                        class: "searchable-select-option disabled",
                                        "No currencies match"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            div {
                class: "modal-footer",
                button {
                    class: "btn btn-outline",
                    r#type: "button",
                    onclick: move |_| on_cancel.call(()),
                    "Cancel"
                }
                button {
                    class: "btn btn-primary",
                    r#type: "submit",
                    "{submit_label}"
                }
            }
        }
    }
}
