//! Landing page.
//!
//! Deliberately more than a static placeholder. The "Check server" control
//! calls a real server function, which means this one page exercises the
//! whole architecture end to end: the component renders on the server,
//! hydrates in the browser, dispatches an `Action`, and the `Action`'s
//! `.pending()` drives `PendingButton`. If any of those four were wired
//! wrongly this button would not work, and nothing else here would say so.
//!
//! It is also what `e2e/tests/design-system.spec.ts` measures. A design
//! system with no page to measure is a document.

use crate::ui::PendingButton;
use leptos::prelude::*;

/// Proves the server half is reachable from the hydrated half.
///
/// Returns the state the portal can actually vouch for. It will grow as the
/// portal does -- `A4` adds a reachability check for `caldav-rs`'s admin API,
/// which is the next thing whose absence should be visible here rather than
/// discovered at the first sign-up.
#[server]
pub async fn server_status() -> Result<String, ServerFnError> {
    Ok(format!("portal {} ready", env!("CARGO_PKG_VERSION")))
}

#[component]
pub fn Home() -> impl IntoView {
    let check = Action::new(|(): &()| async { server_status().await });
    let pending = check.pending();
    let value = check.value();

    view! {
        <h1 class="text-title font-semibold mb-2">"Your calendars"</h1>
        <p class="text-body text-ink-48 mb-6">
            "Connect Notion, pick a database, and get CalDAV credentials for it."
        </p>

        <div class="rounded-lg bg-canvas border border-hairline p-5">
            <p class="text-body mb-4">"Signing in is not wired up yet."</p>
            <PendingButton
                pending=pending
                label="Check server"
                busy="Checking"
                testid="check-server"
                class="w-full h-12 inline-flex items-center justify-center gap-2
                       rounded-full bg-action text-canvas text-body font-semibold
                       disabled:opacity-40 disabled:cursor-not-allowed"
                on_click=Callback::new(move |()| {
                    check.dispatch(());
                })
            />
            <p data-testid="status" class="mt-4 text-caption text-ink-48" aria-live="polite">
                {move || match value.get() {
                    None => String::new(),
                    Some(Ok(s)) => s,
                    Some(Err(e)) => format!("unreachable: {e}"),
                }}
            </p>
        </div>
    }
}
