//! Navigation component for sidebar navigation.
//!
//! Provides a navigation menu with sections and items.

use leptos::prelude::*;

/// A navigation section with a title and items.
#[component]

pub fn NavSection(
    /// The section title.
    #[prop(into)]
    title: String,
    /// The navigation items.
    children: Children,
) -> impl IntoView {
    view! {
        <div class="nav-section">
            <h3 class="nav-section-title">{title}</h3>
            <nav class="nav-section-items">
                {children()}
            </nav>
        </div>
    }
}

/// A navigation item with icon and label.
#[component]

pub fn NavItem(
    /// The item label.
    #[prop(into)]
    label: String,
    /// Whether this item is currently active/selected.
    #[prop(default = false)]
    active: bool,
    /// Icon to display (as SVG path data).
    #[prop(optional, into)]
    icon: Option<String>,
    /// Click handler.
    #[prop(optional)]
    on_click: Option<Callback<()>>,
) -> impl IntoView {
    let handle_click = move |_| {
        if let Some(callback) = on_click {
            callback.run(());
        }
    };

    view! {
        <button
            class="nav-item"
            class:active=active
            on:click=handle_click
        >
            {icon.map(|path| view! {
                <svg class="nav-item-icon" viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                    <path d=path />
                </svg>
            })}
            <span class="nav-item-label">{label}</span>
        </button>
    }
}

/// Common icon paths for navigation items.
pub mod icons {
    /// Home icon.
    pub const HOME: &str = "M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z";
    /// Playlists icon.
    pub const PLAYLISTS: &str = "M15 6H3v2h12V6zm0 4H3v2h12v-2zM3 16h8v-2H3v2zM17 6v8.18c-.31-.11-.65-.18-1-.18-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3V8h3V6h-5z";
    /// Devices icon.
    pub const DEVICES: &str = "M4 6h18V4H4c-1.1 0-2 .9-2 2v11H0v3h14v-3H4V6zm19 2h-6c-.55 0-1 .45-1 1v10c0 .55.45 1 1 1h6c.55 0 1-.45 1-1V9c0-.55-.45-1-1-1zm-1 9h-4v-7h4v7z";
    /// Settings icon.
    pub const SETTINGS: &str = "M19.14 12.94c.04-.31.06-.63.06-.94 0-.31-.02-.63-.06-.94l2.03-1.58c.18-.14.23-.41.12-.61l-1.92-3.32c-.12-.22-.37-.29-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54c-.04-.24-.24-.41-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.04.31-.06.63-.06.94s.02.63.06.94l-2.03 1.58c-.18.14-.23.41-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z";
    /// Download icon.
    pub const DOWNLOAD: &str = "M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z";
    /// Sync icon.
    pub const SYNC: &str = "M12 4V1L8 5l4 4V6c3.31 0 6 2.69 6 6 0 1.01-.25 1.97-.7 2.8l1.46 1.46C19.54 15.03 20 13.57 20 12c0-4.42-3.58-8-8-8zm0 14c-3.31 0-6-2.69-6-6 0-1.01.25-1.97.7-2.8L5.24 7.74C4.46 8.97 4 10.43 4 12c0 4.42 3.58 8 8 8v3l4-4-4-4v3z";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icons_are_valid() {
        // Ensure all icons are non-empty
        assert!(!icons::HOME.is_empty());
        assert!(!icons::PLAYLISTS.is_empty());
        assert!(!icons::DEVICES.is_empty());
        assert!(!icons::SETTINGS.is_empty());
    }
}
