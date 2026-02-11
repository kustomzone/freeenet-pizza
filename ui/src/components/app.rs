//! Main Application Component

use crate::components::{NewOrderDialog, OrderView, Sidebar};
use crate::services::{init_identity, AppState};
use dioxus::prelude::*;

/// Main application component
#[component]
pub fn App() -> Element {
    // Initialize identity on first render
    let mut identity = use_signal(|| init_identity());

    // Application state
    let mut app_state = use_signal(|| AppState::load());

    // Currently selected order
    let mut selected_order_id = use_signal(|| None::<String>);

    // Dialog visibility
    let mut show_new_order_dialog = use_signal(|| false);

    rsx! {
        div { class: "app-container",
            Sidebar {
                app_state: app_state,
                selected_order_id: selected_order_id,
                on_select: move |id: String| {
                    selected_order_id.set(Some(id));
                },
                on_new_order: move |_| {
                    show_new_order_dialog.set(true);
                },
            }

            div { class: "main-content",
                if let Some(order_id) = selected_order_id.read().clone() {
                    OrderView {
                        order_id: order_id,
                        app_state: app_state,
                        identity: identity,
                    }
                } else {
                    div { class: "empty-state",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "80",
                            height: "80",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "1.5",
                            circle { cx: "12", cy: "12", r: "10" }
                            path { d: "M8 12h8M12 8v8" }
                        }
                        h3 { "No order selected" }
                        p { "Select an order from the sidebar or create a new one" }
                    }
                }
            }

            if *show_new_order_dialog.read() {
                NewOrderDialog {
                    on_create: move |name: String| {
                        let id = app_state.write().create_order(name, identity.read().signing_key());
                        selected_order_id.set(Some(id));
                        show_new_order_dialog.set(false);
                    },
                    on_close: move |_| {
                        show_new_order_dialog.set(false);
                    },
                }
            }
        }
    }
}
