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
                    select {
                        value: "{currency}",
                        onchange: move |e| currency.set(e.value()),
                        for curr in COMMON_CURRENCIES.iter() {
                            option {
                                value: "{curr.iso_alpha_code}",
                                selected: curr.iso_alpha_code == currency.read().as_str(),
                                "{curr.iso_alpha_code} {curr.symbol} - {curr.name}"
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
