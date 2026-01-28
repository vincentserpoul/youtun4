//! Button component.

use leptos::prelude::*;

/// Button variant styles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Primary action button.
    #[default]
    Primary,
    /// Secondary action button.
    Secondary,
    /// Danger/destructive action.
    Danger,
    /// Ghost/minimal button.
    Ghost,
}

impl ButtonVariant {
    /// Get CSS classes for this variant.
    #[must_use]
    pub const fn classes(self) -> &'static str {
        match self {
            Self::Primary => "btn btn-primary",
            Self::Secondary => "btn btn-secondary",
            Self::Danger => "btn btn-danger",
            Self::Ghost => "btn btn-ghost",
        }
    }
}

/// Reusable button component.
#[component]
pub fn Button(
    /// Button text content.
    children: Children,
    /// Click handler.
    #[prop(optional)] on_click: Option<Callback<()>>,
    /// Button variant.
    #[prop(default = ButtonVariant::Primary)] variant: ButtonVariant,
    /// Whether the button is disabled.
    #[prop(default = false)] disabled: bool,
    /// Whether the button is in loading state.
    #[prop(default = false)] loading: bool,
) -> impl IntoView {
    let is_disabled = disabled || loading;
    let classes = variant.classes();

    view! {
        <button
            class=classes
            disabled=is_disabled
            on:click=move |_| {
                if let Some(handler) = &on_click {
                    handler.run(());
                }
            }
        >
            {if loading {
                view! { <span class="spinner"></span> }.into_any()
            } else {
                children().into_any()
            }}
        </button>
    }
}
