use dioxus::prelude::*;
use std::collections::HashMap;

use crate::api::{
    connect_node_api, get_auth_token_from_window, NodeConfig, AUTH_TOKEN, NODE_HTTP_BASE,
};
use crate::components::{NewOrderDialog, OrderSettings, OrderViewComponent, Sidebar};
pub(crate) use crate::services::{BaseService, Contract, FreenetService};
use chrono::Utc;
use ed25519_dalek::{SigningKey, VerifyingKey};
use futures::{FutureExt, StreamExt};
use pizza_common::order_state::*;

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
        let api_url =
            api_url.replace("http", "ws") + "/v1/contract/command?encodingProtocol=native";
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
    let base_for_delete = base.clone();
    let sk = base.get_private_key().unwrap();
    let sk_signal = use_context_provider(|| Signal::new(sk.clone()));

    let mut contracts = use_context_provider(|| Signal::new(HashMap::<String, Contract>::new()));
    let mut show_new_order = use_signal(|| false);
    let mut loading = use_signal(|| true);

    use_effect(move || {
        let base = base.clone();
        spawn(async move {
            // Initial load from localStorage keys - load pessimistically with timeouts
            if let Ok(ids) = base.get_contracts() {
                // Load contracts incrementally with individual timeouts
                // This prevents hanging if network requests fail
                for id in ids.iter() {
                    let base = base.clone();
                    let id = id.clone();
                    spawn(async move {
                        // Use a timeout for each contract load (5 seconds)
                        let load_future = base.get_contract(id.clone());
                        let timeout_future = gloo_timers::future::TimeoutFuture::new(5_000);

                        // Race between load and timeout
                        futures::select! {
                            result = load_future.fuse() => {
                                if let Ok(contract) = result {
                                    // Update contracts incrementally as each loads
                                    let mut current = contracts.peek().clone();
                                    current.insert(id.clone(), Contract {
                                        state: contract.state,
                                        parameters: contract.parameters,
                                    });
                                    contracts.set(current);

                                    // Subscribe to state updates for this contract
                                    let base = base.clone();
                                    let id_clone = id.clone();
                                    spawn(async move {
                                        let mut stream = base.subscribe_contract_state(id_clone.clone());
                                        while let Some(updated) = stream.next().await {
                                            let mut current = contracts.peek().clone();
                                            if let Some(existing) = current.get_mut(&id_clone) {
                                                existing.state = updated.state;
                                                existing.parameters = updated.parameters;
                                                contracts.set(current);
                                            }
                                        }
                                    });
                                }
                            }
                            _ = timeout_future.fuse() => {
                                // Timeout - contract load hung, skip this contract
                                log::warn!("Timeout loading contract: {}", id);
                            }
                        }
                    });
                }
            }

            // Set loading to false after a brief delay to allow initial loads to complete
            gloo_timers::future::TimeoutFuture::new(500).await;
            loading.set(false);

            // Subscribe to contract list changes
            let mut stream = base.subscribe_contracts();
            while let Some(ids) = stream.next().await {
                let mut current_contracts = contracts.peek().clone();
                let mut changed = false;

                // Remove contracts that are no longer present
                current_contracts.retain(|id, _| ids.contains(id));

                for id in ids {
                    if !current_contracts.contains_key(&id) {
                        // Fetch new contract (tries cache first, then network)
                        if let Ok(contract) = base.get_contract(id.clone()).await {
                            let vk = contract.parameters.owner;
                            current_contracts.insert(
                                id.clone(),
                                Contract {
                                    state: contract.state,
                                    parameters: contract.parameters,
                                },
                            );
                            changed = true;

                            // Subscribe to state updates for the new contract
                            let base = base.clone();
                            let id_clone = id.clone();
                            spawn(async move {
                                let mut stream = base.subscribe_contract_state(id_clone.clone());
                                while let Some(updated) = stream.next().await {
                                    let mut current = contracts.peek().clone();
                                    if let Some(existing) = current.get_mut(&id_clone) {
                                        existing.state = updated.state;
                                        existing.parameters = updated.parameters;
                                        contracts.set(current);
                                    }
                                }
                            });
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
                on_delete: move |id: String| {
                    base_for_delete.remove_contract(id.clone());
                    // Also remove from local contracts signal
                    let mut current = contracts.peek().clone();
                    current.remove(&id);
                    contracts.set(current);
                },
                selected_order_id: selected_order_id,
                loading: loading()
            }
            main {
                Outlet::<Route> {}
            }

            if show_new_order() {
                NewOrderDialog {
                    sk: sk_signal,
                    on_create: move |settings: OrderSettings| {
                        let base = base_for_dialog.clone();
                        let sk_val = sk_signal.read().clone();
                        let nav = navigator();
                        let order = AuthorizedOrderV1::new(Order { name: settings.name, currency: settings.currency, order_version: 1 }, &sk_val);
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
