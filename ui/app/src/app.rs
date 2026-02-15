use dioxus::prelude::*;
use dioxus::prelude::Router;

use crate::components::{NewOrderDialog, OrderViewComponent, Sidebar};
use pizza_common::order_state::*;
use chrono::Utc;

use ed25519_dalek::SigningKey;
use ed25519_dalek::VerifyingKey;

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
        OrderViewComponent { contracts: use_context::<Signal<Vec<Contract>>>(), sk: use_context::<Signal<SigningKey>>(), id: id }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Contract {
    pub state: FullOrderStateV1,
    pub parameters: OrderParametersV1,
    pub sk: Option<SigningKey>,
    pub vk: VerifyingKey,
    pub id: String,
}

#[component]
pub fn App() -> Element {
    rsx! {
        Router::<Route> {}
    }
}

#[component]
fn AppContent() -> Element {
    let mut bytes = [0u8; 32];
    rand::Rng::fill(&mut rand::thread_rng(), &mut bytes);
    let sk = use_signal(|| SigningKey::from_bytes(&bytes));

    let mut contracts = use_signal(|| Vec::<Contract>::new());
    let mut show_new_order = use_signal(|| false);

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
                sk: sk,
                on_new_order: move |_| show_new_order.set(true),
                selected_order_id: selected_order_id
            }
            main {
                match route {
                    Route::OrderPage { id: ref_id } => rsx! {
                        OrderViewComponent { contracts: contracts, sk: sk, id: ref_id.clone() }
                    },
                    _ => rsx! {
                        Outlet::<Route> {}
                    }
                }
            }

            if show_new_order() {
                NewOrderDialog {
                    _sk: sk,
                    on_create: move |name: String| {
                        let id = format!("{}", rand::random::<u32>());
                        let sk_val = sk.read().clone();
                        let order = AuthorizedOrderV1::new(Order { name, order_version: 1 }, &sk_val);
                        let paid = AuthorizedPaidV1::new(Paid::default(), &sk_val);
                        let vk = sk_val.verifying_key();
                        let contract = Contract {
                            state: FullOrderStateV1 {
                                order: order,
                                items: ItemsV1::default(),
                                paid: paid,
                                ..Default::default()
                            },
                            parameters: OrderParametersV1 {
                                owner: vk,
                                created_at: Utc::now(),
                            },
                            id: id.clone(),
                            sk: Some(sk_val),
                            vk,
                        };
                        contracts.with_mut(|c| c.push(contract));
                        show_new_order.set(false);
                        let nav = use_navigator();
                        nav.push(Route::OrderPage { id });
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
