//! Toast notification component for displaying feedback messages.
//!
//! Provides a toast notification system with support for different notification
//! types (info, success, warning, error) and automatic dismissal.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::types::{Notification, NotificationType};

/// Context for managing notifications across the application.
#[derive(Clone, Copy)]
pub struct NotificationContext {
    /// Current list of notifications.
    pub notifications: ReadSignal<Vec<Notification>>,
    /// Signal to update the notifications list.
    set_notifications: WriteSignal<Vec<Notification>>,
}

impl NotificationContext {
    /// Create a new notification context.
    #[must_use]
    pub fn new() -> Self {
        let (notifications, set_notifications) = signal::<Vec<Notification>>(vec![]);
        Self {
            notifications,
            set_notifications,
        }
    }

    /// Add a notification to the stack.
    pub fn push(&self, notification: Notification) {
        let id = notification.id;
        let duration_ms = notification.duration_ms;
        let set_notifications = self.set_notifications;

        // Add the notification
        self.set_notifications.update(|notifications| {
            notifications.push(notification);
        });

        // Set up auto-dismiss if duration is specified
        if let Some(duration) = duration_ms {
            spawn_local(async move {
                gloo_timers::future::TimeoutFuture::new(duration as u32).await;
                set_notifications.update(|notifications| {
                    notifications.retain(|n| n.id != id);
                });
            });
        }
    }

    /// Remove a notification by ID.
    pub fn dismiss(&self, id: u64) {
        self.set_notifications.update(|notifications| {
            notifications.retain(|n| n.id != id);
        });
    }

    /// Show an info notification.
    pub fn info(&self, message: impl Into<String>) {
        self.push(Notification::info(message));
    }

    /// Show a success notification.
    pub fn success(&self, message: impl Into<String>) {
        self.push(Notification::success(message));
    }

    /// Show a warning notification.
    pub fn warning(&self, message: impl Into<String>) {
        self.push(Notification::warning(message));
    }

    /// Show an error notification.
    pub fn error(&self, message: impl Into<String>) {
        self.push(Notification::error(message));
    }

    /// Clear all notifications.
    pub fn clear_all(&self) {
        self.set_notifications.set(vec![]);
    }
}

impl Default for NotificationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Container for displaying toast notifications.
///
/// This component should be placed at the root level of the application
/// to display notifications from anywhere in the component tree.
#[component]

pub fn ToastContainer() -> impl IntoView {
    let ctx = expect_context::<NotificationContext>();

    view! {
        <div class="toast-container" data-testid="toast-container">
            <For
                each=move || ctx.notifications.get()
                key=|notification| notification.id
                children=move |notification| {
                    let id = notification.id;
                    view! {
                        <Toast
                            notification=notification
                            on_dismiss=Callback::new(move |()| {
                                ctx.dismiss(id);
                            })
                        />
                    }
                }
            />
        </div>
    }
}

/// A single toast notification.
#[component]
fn Toast(
    /// The notification to display.
    notification: Notification,
    /// Callback when the notification is dismissed.
    on_dismiss: Callback<()>,
) -> impl IntoView {
    let notification_type = notification.notification_type;
    let type_class = format!("toast-{notification_type}");
    let has_title = notification.title.is_some();

    view! {
        <div
            class=format!("toast {}", type_class)
            role="alert"
            aria-live="polite"
            data-testid="toast"
            data-toast-type=notification_type.to_string()
        >
            <div class="toast-icon">
                {move || match notification_type {
                    NotificationType::Info => view! {
                        <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z"/>
                        </svg>
                    }.into_any(),
                    NotificationType::Success => view! {
                        <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
                        </svg>
                    }.into_any(),
                    NotificationType::Warning => view! {
                        <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                            <path d="M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z"/>
                        </svg>
                    }.into_any(),
                    NotificationType::Error => view! {
                        <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
                        </svg>
                    }.into_any(),
                }}
            </div>
            <div class="toast-content">
                {notification.title.clone().map(|title| view! {
                    <div class="toast-title">{title}</div>
                })}
                <div class="toast-message" class:has-title=has_title>
                    {notification.message}
                </div>
            </div>
            <button
                class="toast-dismiss btn btn-ghost btn-icon"
                on:click=move |_| on_dismiss.run(())
                aria-label="Dismiss notification"
                data-testid="toast-dismiss"
            >
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                    <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                </svg>
            </button>
        </div>
    }
}

/// Provider component that sets up the notification context.
///
/// Wrap your application with this component to enable notifications.
#[component]

pub fn NotificationProvider(
    /// Child components that can access the notification context.
    children: Children,
) -> impl IntoView {
    let ctx = NotificationContext::new();
    provide_context(ctx);

    view! {
        {children()}
        <ToastContainer />
    }
}

/// Hook to access the notification context.
///
/// # Panics
/// Panics if called outside of a `NotificationProvider`.
pub fn use_notifications() -> NotificationContext {
    expect_context::<NotificationContext>()
}
