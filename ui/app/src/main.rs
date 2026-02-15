//! Pizza Order Manager UI
//!
//! A Dioxus-based frontend for managing collaborative pizza orders on Freenet.

mod components;
mod app;
mod services;
mod api;

use dioxus::prelude::*;

fn main() {
    // Set up logging and panic hook for WASM
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        let _ = console_log::init_with_level(log::Level::Debug);
    }

    // Launch the Dioxus app
    launch(app::App);
}
