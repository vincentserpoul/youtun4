//! Header component.

use leptos::prelude::*;

/// Application header component.
#[component]

pub fn Header() -> impl IntoView {
    view! {
        <header class="app-header">
            <div class="logo">
                <svg viewBox="0 0 24 24" width="32" height="32" fill="var(--accent-primary)">
                    <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
                </svg>
                <span class="logo-text">"Youtun4"</span>
            </div>
        </header>
    }
}
