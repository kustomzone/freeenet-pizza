use std::collections::HashMap;
use dioxus::prelude::*;

use crate::components::{NewOrderDialog, OrderViewComponent, Sidebar};
use pizza_common::order_state::*;
use chrono::Utc;
use crate::services::{FreenetService, BaseService};
use ed25519_dalek::{SigningKey, VerifyingKey};
use futures::StreamExt;
use crate::api::{get_auth_token_from_window, NodeConfig, connect_node_api, NODE_HTTP_BASE, AUTH_TOKEN};

#[derive(Clone, Routable, Debug, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppContent)]
        #[route("/")]
        HomePage {},
        #[route("/order/:id")]
        OrderPage { id: String },
    #[end_layout]
    #[route("/not-found/:..route")]
    PageNotFound { route: Vec<String> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Contract {
    pub state: FullOrderStateV1,
    pub parameters: OrderParametersV1,
    pub sk: Option<SigningKey>,
    pub vk: VerifyingKey,
    pub id: String,
}

impl From<Contract> for crate::services::Contract {
    fn from(c: Contract) -> Self {
        Self {
            state: c.state,
            parameters: c.parameters,
        }
    }
}

#[component]
fn OrderPage(id: String) -> Element {
    rsx! {
        OrderViewComponent { id: id }
    }
}

#[component]
pub fn App() -> Element {
    use_context_provider(|| {
        let storage = FreenetService::new().expect("Failed to create FreenetService");
        BaseService(std::rc::Rc::new(storage))
    });
    get_auth_token_from_window();

    use_effect(|| {
        let api_url = NODE_HTTP_BASE.read().clone();
        let auth_token = AUTH_TOKEN.read().clone();
        let api_url = api_url.replace("http", "ws") + "/v1/contract/command?encodingProtocol=native";
        connect_node_api(&NodeConfig { api_url });
    });

    rsx! {
        Router::<Route> {}
    }
}

#[component]
fn AppContent() -> Element {
    let base = use_context::<BaseService>();
    let base_for_dialog = base.clone();
    let sk = base.get_private_key().unwrap();
    let sk_signal = use_context_provider(|| Signal::new(sk.clone()));

    let mut contracts = use_context_provider(|| Signal::new(HashMap::<String, Contract>::new()));
    let mut show_new_order = use_signal(|| false);

    use_effect(move || {
        let base = base.clone();
        spawn(async move {
            // Initial load
            if let Ok(ids) = base.get_contracts() {
                let mut loaded_contracts = HashMap::new();
                for id in ids {
                    if let Ok(contract) = base.get_contract_parameters_and_state(id.clone()) {
                        let vk = contract.parameters.owner;
                        loaded_contracts.insert(id.clone(), Contract {
                            state: contract.state,
                            parameters: contract.parameters,
                            sk: None,
                            vk,
                            id,
                        });
                    }
                }
                contracts.set(loaded_contracts);
            }

            // Subscribe to changes
            let mut stream = base.subscribe_contracts();
            while let Some(ids) = stream.next().await {
                let mut current_contracts = contracts.peek().clone();
                let mut changed = false;
                
                // Remove contracts that are no longer present
                current_contracts.retain(|id, _| ids.contains(id));
                
                for id in ids {
                    if !current_contracts.contains_key(&id) {
                        if let Ok(contract) = base.get_contract_parameters_and_state(id.clone()) {
                            let vk = contract.parameters.owner;
                            current_contracts.insert(id.clone(), Contract {
                                state: contract.state,
                                parameters: contract.parameters,
                                sk: None,
                                vk,
                                id,
                            });
                            changed = true;
                        }
                    }
                }
                if changed {
                    contracts.set(current_contracts);
                }
            }
        });
    });

    let route = use_route::<Route>();
    let selected_order_id = match route {
        Route::OrderPage { ref id } => Some(id.clone()),
        _ => None,
    };

    rsx! {
        Stylesheet { href: asset!("/assets/main.css") }

        div {
            class: "app-container",
            Sidebar {
                contracts: contracts,
                sk: sk_signal,
                on_new_order: move |_| show_new_order.set(true),
                selected_order_id: selected_order_id
            }
            main {
                Outlet::<Route> {}
            }

            if show_new_order() {
                NewOrderDialog {
                    sk: sk_signal,
                    on_create: move |name: String| {
                        let base = base_for_dialog.clone();
                        let sk_val = sk_signal.read().clone();
                        let nav = navigator();
                        let order = AuthorizedOrderV1::new(Order { name, currency: "$".parse().unwrap(), order_version: 1 }, &sk_val);
                        let parameters = OrderParametersV1 {
                            owner: sk_val.verifying_key(),
                            created_at: Utc::now(),
                        };
                        let state = FullOrderStateV1 {
                            order: order,
                            items: ItemsV1::default(),
                            paid: AuthorizedPaidV1::new(Paid::default(), &sk_val),
                            ..Default::default()
                        };

                        let contract = crate::services::Contract {
                            state,
                            parameters,
                        };

                        // Spawn async task to publish the contract
                        spawn(async move {
                            if let Ok(res) = base.publish_contract(contract).await {
                                show_new_order.set(false);
                                // Navigate to the newly created order
                                nav.push(Route::OrderPage { id: res.contract_key });
                            }
                        });
                    },
                    on_close: move |_| show_new_order.set(false)
                }
            }
        }
    }
}

#[component]
fn HomePage() -> Element {
    rsx! {
        div {
            class: "empty-state",
            h3 { "No order selected" }
            p { "Select an order from the sidebar or create a new one" }
        }
    }
}

#[component]
fn PageNotFound(route: Vec<String>) -> Element {
    rsx! {
        "Page not found."
    }
}
