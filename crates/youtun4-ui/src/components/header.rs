//! Header component.

use leptos::prelude::*;

/// Application header component.
#[component]

pub fn Header() -> impl IntoView {
    view! {
        <header class="app-header">
            <div class="logo">
                <span class="logo-text">"Youtun4"</span>
            </div>
        </header>
    }
}
