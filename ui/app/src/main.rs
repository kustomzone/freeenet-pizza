//! Pizza Order Manager UI
//!
//! A Dioxus-based frontend for managing collaborative pizza orders on Freenet.

mod components;
mod app;
mod services;
mod api;

use std::rc::Rc;
use dioxus::prelude::*;

fn main() {
    // Set up logging and panic hook for WASM
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        let _ = console_log::init_with_level(log::Level::Debug);
    }

    // Launch the Dioxus app with HashHistory for proper routing on Freenet
    // HashHistory uses URL fragments (#/path) instead of full paths,
    // which works correctly when served from a Freenet contract
    dioxus::LaunchBuilder::new()
        .with_cfg(dioxus::web::Config::new().history(Rc::new(dioxus::web::HashHistory::default())))
        .launch(app::App);
}
