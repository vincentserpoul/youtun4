//! Layout component for the application structure.
//!
//! Provides a responsive layout with header, navigation sidebar, and main content area.

use leptos::prelude::*;

/// Provides access to the mobile menu state for child components.
#[derive(Clone, Copy)]
pub struct MobileMenuContext {
    /// Whether the mobile menu is currently open.
    pub is_open: ReadSignal<bool>,
    /// Set the mobile menu open state.
    pub set_open: WriteSignal<bool>,
}

impl MobileMenuContext {
    /// Toggle the mobile menu.
    pub fn toggle(&self) {
        self.set_open.update(|open| *open = !*open);
    }

    /// Close the mobile menu.
    pub fn close(&self) {
        self.set_open.set(false);
    }
}

/// Context for settings panel open state.
#[derive(Clone, Copy)]
pub struct SettingsContext {
    /// Whether settings panel is open.
    pub is_open: ReadSignal<bool>,
    /// Set settings open state.
    pub set_open: WriteSignal<bool>,
}

/// Type alias for header actions function.
pub type HeaderActionsFn = Box<dyn FnOnce() -> AnyView + Send>;

/// The main layout component that provides the application structure.
///
/// This component creates a responsive layout with:
/// - A fixed header at the top
/// - A collapsible sidebar for navigation (hidden on mobile)
/// - A main content area that fills the remaining space
/// - Mobile navigation overlay support
///
/// Use `LayoutSidebar` and `LayoutMain` as children to populate the layout.
#[component]

pub fn Layout(
    /// Content for the layout (should use LayoutSidebar and LayoutMain).
    children: Children,
    /// Callback when settings button is clicked.
    #[prop(optional)]
    on_settings_click: Option<Callback<()>>,
) -> impl IntoView {
    // Mobile menu state
    let (mobile_menu_open, set_mobile_menu_open) = signal(false);

    // Provide context for child components
    provide_context(MobileMenuContext {
        is_open: mobile_menu_open,
        set_open: set_mobile_menu_open,
    });

    // Close menu when clicking overlay
    let close_menu = move |_| {
        set_mobile_menu_open.set(false);
    };

    // Toggle menu
    let toggle_menu = move |_| {
        set_mobile_menu_open.update(|open| *open = !*open);
    };

    // Handle settings click
    let handle_settings_click = move |_| {
        if let Some(cb) = on_settings_click {
            cb.run(());
        }
    };

    view! {
        <div class="layout">
            // Header with mobile menu toggle and window drag region
            <header class="layout-header" data-tauri-drag-region="true">
                <button
                    class="layout-menu-toggle btn btn-ghost btn-icon"
                    on:click=toggle_menu
                    aria-label="Toggle menu"
                    aria-expanded=move || mobile_menu_open.get().to_string()
                >
                    <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
                        {move || if mobile_menu_open.get() {
                            // X icon when menu is open
                            view! {
                                <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                            }.into_any()
                        } else {
                            // Hamburger icon when menu is closed
                            view! {
                                <path d="M3 18h18v-2H3v2zm0-5h18v-2H3v2zm0-7v2h18V6H3z"/>
                            }.into_any()
                        }}
                    </svg>
                </button>
                <div class="logo">
                    <span class="logo-text">"Youtun4"</span>
                </div>
                // Header actions (settings button)
                <div class="layout-header-actions">
                    {on_settings_click.is_some().then(|| view! {
                        <button
                            class="btn btn-ghost btn-icon"
                            title="Settings"
                            on:click=handle_settings_click
                        >
                            <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
                                <path d="M19.14 12.94c.04-.31.06-.63.06-.94 0-.31-.02-.63-.06-.94l2.03-1.58c.18-.14.23-.41.12-.61l-1.92-3.32c-.12-.22-.37-.29-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54c-.04-.24-.24-.41-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.04.31-.06.63-.06.94 0 .31.02.63.06.94l-2.03 1.58c-.18.14-.23.41-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z"/>
                            </svg>
                        </button>
                    })}
                </div>
            </header>

            // Mobile overlay
            <div
                class="layout-overlay"
                class:visible=move || mobile_menu_open.get()
                on:click=close_menu
            ></div>

            // Main container with sidebar and content
            <div class="layout-container">
                {children()}
            </div>
        </div>
    }
}

/// Sidebar component for use within Layout.
#[component]

pub fn LayoutSidebar(
    /// Content to render in the sidebar.
    children: Children,
) -> impl IntoView {
    let menu_ctx = expect_context::<MobileMenuContext>();

    view! {
        <aside
            class="layout-sidebar"
            class:open=move || menu_ctx.is_open.get()
        >
            {children()}
        </aside>
    }
}

/// Main content area component for use within Layout.
#[component]

pub fn LayoutMain(
    /// Content to render in the main area.
    children: Children,
) -> impl IntoView {
    view! {
        <main class="layout-content">
            {children()}
        </main>
    }
}

/// A section within the content area with optional header.
#[component]

pub fn ContentSection(
    /// Optional title for the section.
    #[prop(optional, into)]
    title: Option<String>,
    /// The content of the section.
    children: Children,
) -> impl IntoView {
    view! {
        <section class="content-section">
            {title.map(|t| {
                view! {
                    <div class="content-section-header">
                        <h2>{t}</h2>
                    </div>
                }
            })}
            <div class="content-section-body">
                {children()}
            </div>
        </section>
    }
}

/// Header for a content section with title and actions.
#[component]

pub fn ContentHeader(
    /// Title text.
    #[prop(into)]
    title: String,
    /// Action buttons or other content.
    children: Children,
) -> impl IntoView {
    view! {
        <div class="content-section-header">
            <h2>{title}</h2>
            <div class="content-section-actions">
                {children()}
            </div>
        </div>
    }
}

/// Header actions component for placing actions in the layout header.
/// This should be placed inside a Layout to add buttons to the header.
#[component]

pub fn LayoutHeaderActions(
    /// Action buttons or other header content.
    children: Children,
) -> impl IntoView {
    view! {
        <div class="layout-header-actions-content">
            {children()}
        </div>
    }
}

/// A responsive grid for displaying cards or items.
#[component]

pub fn ResponsiveGrid(
    /// Minimum width of each item in the grid.
    #[prop(default = "300px".to_string(), into)]
    min_item_width: String,
    /// The grid items.
    children: Children,
) -> impl IntoView {
    let style = format!("--grid-min-width: {min_item_width}");

    view! {
        <div class="responsive-grid" style=style>
            {children()}
        </div>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_layout_compiles() {
        // Basic compile test - actual rendering tests done via Playwright
    }
}
