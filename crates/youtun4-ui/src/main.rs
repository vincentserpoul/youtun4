//! Youtun4 UI entry point for WASM.

#![no_main]

use leptos::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;
use youtun4_ui::App;

/// Entry point for the WASM application.
/// This function is called automatically when the WASM module is loaded.
#[wasm_bindgen(start)]
pub fn start() {
    // Set up better panic messages in the browser console
    console_error_panic_hook::set_once();

    // Remove the loading spinner
    if let Some(window) = web_sys::window()
        && let Some(document) = window.document()
        && let Some(loading) = document.get_element_by_id("loading")
    {
        loading.remove();
    }

    // Mount the Leptos app to the DOM
    mount_to_body(App);
}
