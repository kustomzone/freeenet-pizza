use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

pub mod components;
use crate::components::{NewOrderDialog, OrderView, PizzaOrder, Sidebar};
use leptos_router::hooks::{use_navigate, use_params_map};

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    let orders = RwSignal::new(vec![
        PizzaOrder {
            id: "1".to_string(),
            name: "Test Order".to_string(),
            item_count: 2,
            created_at: "2024-02-14".to_string(),
        },
    ]);
    let show_new_order = RwSignal::new(false);

    view! {
        // sets the document title
        <Title text="Pizza Freenet"/>

        // content for this welcome page
        <Router>
            <AppContent orders show_new_order />
        </Router>
    }
}

#[component]
fn AppContent(orders: RwSignal<Vec<PizzaOrder>>, show_new_order: RwSignal<bool>) -> impl IntoView {
    let navigate = use_navigate();
    // let params = use_params_map();

    view! {
        <div class="app-container">
            <Sidebar
                orders=orders.into()
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
                    on_create=Callback::new({
                        let navigate = navigate.clone();
                        move |name: String| {
                            let id = format!("{}", rand::random::<u32>());
                            orders.update(|os| os.push(PizzaOrder {
                                id: id.clone(),
                                name: name.clone(),
                                item_count: 0,
                                created_at: "Just now".to_string(),
                            }));
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
