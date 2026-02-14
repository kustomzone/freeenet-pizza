use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

pub mod components;
use crate::components::{PizzaOrder, Sidebar};

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
    let selected_order_id = RwSignal::new(None::<String>);

    view! {
        // sets the document title
        <Title text="Pizza Freenet"/>

        // content for this welcome page
        <Router>
            <div class="app-container">
                <Sidebar
                    orders=orders.into()
                    selected_order_id=selected_order_id.into()
                    on_select=Callback::new(move |id| selected_order_id.set(Some(id)))
                    on_new_order=Callback::new(move |_| println!("New order clicked"))
                />
                <main>
                    <Routes fallback=|| "Page not found.".into_view()>
                        <Route path=StaticSegment("") view=HomePage/>
                    </Routes>
                </main>
            </div>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    // Creates a reactive value to update the button
    let count = RwSignal::new(0);
    let on_click = move |_| *count.write() += 1;

    view! {
        <h1>"Welcome to Leptos!"</h1>
        <button on:click=on_click>"Click Me: " {count}</button>
    }
}
