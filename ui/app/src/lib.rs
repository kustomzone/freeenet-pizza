use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

pub mod components;
use crate::components::{NewOrderDialog, OrderView, Sidebar};
use leptos_router::hooks::use_navigate;
use pizza_common::order_state::*;
use chrono::Utc;

use ed25519_dalek::SigningKey;
use ed25519_dalek::VerifyingKey;

#[derive(Clone)]
pub struct Contract {
    pub state: FullOrderStateV1,
    pub parameters: OrderParametersV1,
    pub sk: Option<SigningKey>,
    pub vk: VerifyingKey,
    pub id: String,
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    let mut bytes = [0u8; 32];
    rand::Rng::fill(&mut rand::thread_rng(), &mut bytes);
    let sk = RwSignal::new(SigningKey::from_bytes(&bytes));

    let contracts: RwSignal<Vec<Contract>> = RwSignal::new(vec![]);

    let show_new_order = RwSignal::new(false);

    view! {
        // sets the document title
        <Title text="Pizza Freenet"/>

        // content for this welcome page
        <Router>
            <AppContent contracts show_new_order sk />
        </Router>
    }
}

#[component]
fn AppContent(
    contracts: RwSignal<Vec<Contract>>,
    show_new_order: RwSignal<bool>,
    sk: RwSignal<SigningKey>,
) -> impl IntoView {
    let navigate = use_navigate();
    // let params = use_params_map();

    view! {
        <div class="app-container">
            <Sidebar
                contracts=contracts
                on_new_order=Callback::new(move |_| show_new_order.set(true))
                selected_order_id=Some("da".into()) // params.get().get("id")
            />
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                    <Route path=(StaticSegment("order"), leptos_router::ParamSegment("id")) view=OrderView/>
                </Routes>
            </main>

            <Show when=move || show_new_order.get()>
                <NewOrderDialog
                    _sk=sk
                    on_create=Callback::new({
                        let navigate = navigate.clone();
                        move |name: String| {
                            let id = format!("{}", rand::random::<u32>());
                            let sk_val = sk.get();
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
                            contracts.update(|c| c.push(contract));
                            show_new_order.set(false);
                            navigate(&format!("/order/{}", id), Default::default());
                        }
                    })
                    on_close=Callback::new(move |_| show_new_order.set(false))
                />
            </Show>
        </div>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    view! {
        <div class="empty-state">
            <h3>"No order selected"</h3>
            <p>"Select an order from the sidebar or create a new one"</p>
        </div>
    }
}
