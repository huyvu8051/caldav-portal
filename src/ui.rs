//! Shared feedback primitives.
//!
//! Taken from `webcaldav/src/ui.rs` unchanged. Not "inspired by": the same
//! file, so a fix to the in-flight behaviour in either app is a fix that can
//! be carried across by reading a diff rather than by remembering.
//!
//! Every network call in this app is a `leptos::prelude::Action`, whose
//! `.pending()` memo is the single source of truth for "in flight". These
//! components are the other half: the one agreed way that fact reaches the
//! screen, so no caller has to invent its own.
//!
//! The shape follows Apple's HIG on progress indicators -- an indeterminate
//! activity indicator shown *at the control that started the work*, never a
//! sheet or overlay covering the screen for an operation this short. The
//! control keeps its size and goes disabled, so nothing reflows under a
//! thumb that is still on the glass.

use leptos::prelude::*;

/// Indeterminate activity indicator. Sized in `em`, so it matches whatever
/// text it sits beside; coloured `currentColor`, so it inherits the button's
/// own foreground instead of introducing a second accent.
#[component]
pub fn Spinner(#[prop(optional, into)] class: String) -> impl IntoView {
    view! { <span data-testid="spinner" class=format!("spinner {class}") aria-hidden="true"></span> }
}

/// A button that cannot be pressed twice and says so.
///
/// `pending` comes straight from an action: `on_click` only has to dispatch,
/// and the in-flight state follows on its own rather than being set and
/// cleared by hand on every branch -- which is how the old code lost it (a
/// save that failed early never reset its flag).
#[component]
pub fn PendingButton(
    #[prop(into)] pending: Signal<bool>,
    /// Resting label.
    #[prop(into)]
    label: Signal<String>,
    /// In-flight label. Present tense, because it is describing the wait.
    #[prop(into)]
    busy: Signal<String>,
    #[prop(into)] class: String,
    /// `true` lets a parent `<form>`'s `on:submit` fire. Default is a plain
    /// button that does nothing but call `on_click`.
    #[prop(optional)]
    submit: bool,
    /// Disabling beyond the pending state -- an unsatisfied form, say.
    #[prop(optional, into)]
    disabled: Signal<bool>,
    #[prop(optional)] on_click: Option<Callback<()>>,
    /// Stable hook for the e2e suite. Text and glyphs move around; this does
    /// not, which is the whole reason the tests address it instead.
    #[prop(optional, into)]
    testid: String,
) -> impl IntoView {
    view! {
        <button
            data-testid=testid
            type=move || if submit { "submit" } else { "button" }
            class=class
            // Pressing twice would send a second PUT for the same event.
            disabled=move || pending.get() || disabled.get()
            aria-busy=move || pending.get().to_string()
            on:click=move |_| {
                if let Some(cb) = on_click {
                    cb.run(());
                }
            }
        >
            <Show
                when=move || pending.get()
                fallback=move || view! { <span>{move || label.get()}</span> }
            >
                <Spinner/>
                <span>{move || busy.get()}</span>
            </Show>
        </button>
    }
}

/// Glyph-only button that becomes a spinner in place while its own work runs.
///
/// `pending` is per-row, not per-action: one `Action` serves every event in
/// the list, so the caller narrows it with the action's `input()` to the row
/// actually being deleted. Otherwise one tap spins every row at once.
#[component]
pub fn PendingIconButton(
    #[prop(into)] pending: Signal<bool>,
    #[prop(into)] icon: String,
    #[prop(into)] label: String,
    #[prop(into)] class: String,
    #[prop(optional)] on_click: Option<Callback<()>>,
    #[prop(optional, into)] testid: String,
) -> impl IntoView {
    view! {
        <button
            data-testid=testid
            type="button"
            class=class
            aria-label=label
            disabled=move || pending.get()
            aria-busy=move || pending.get().to_string()
            on:click=move |_| {
                if let Some(cb) = on_click {
                    cb.run(());
                }
            }
        >
            <Show
                when=move || pending.get()
                fallback={
                    let icon = icon.clone();
                    move || {
                        view! {
                            <span class="material-symbols-outlined text-[18px]">
                                {icon.clone()}
                            </span>
                        }
                    }
                }
            >
                <Spinner class="text-[18px]"/>
            </Show>
        </button>
    }
}

/// A link to a route the server handles, not the client router.
///
/// `rel="external"` is load-bearing and its absence is invisible until a
/// human clicks. leptos_router intercepts every same-origin anchor click and
/// resolves it against the *client* route table; `/signin`, `/signout` and
/// `/notion/connect` are plain axum handlers that return 303, so they are not
/// in that table. The router finds no match, renders the NotFound fallback,
/// and the request never reaches the server. The URL bar changes, so it looks
/// like a routing bug in the app rather than a link that was never followed.
///
/// Every curl of those paths works, every unit test passes, and the page says
/// "Not found". Which is how this shipped.
///
/// One component, so the knowledge lives in one place rather than in four
/// anchors that each have to remember.
#[component]
pub fn ServerLink(
    #[prop(into)] href: String,
    #[prop(into)] label: String,
    #[prop(into)] class: String,
    #[prop(optional, into)] testid: String,
    #[prop(optional)] button: bool,
) -> impl IntoView {
    view! {
        <a
            data-testid=testid
            href=href
            rel="external"
            role=move || if button { "button" } else { "link" }
            class=class
        >
            {label}
        </a>
    }
}

/// The reading column.
///
/// 430px, centred, with the page padding. Lifted out of the router's `<main>`
/// so a page can put it *inside* a full-bleed section rather than being
/// wrapped by it -- which is the whole difference between a landing page and
/// a narrow strip of text on a wide screen.
#[component]
pub fn Column(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! {
        <div class=format!("mx-auto max-w-app px-5 {class}")>{children()}</div>
    }
}

/// A labelled value with a copy button.
///
/// Every row on the credential page is something a person has to retype into
/// a phone -- a server URL, a username, a twenty-character password. Copying
/// is the whole interaction, so it is a 44px target rather than a tap on the
/// text.
///
/// The confirmation replaces the glyph in place for a moment rather than
/// appearing beside it: a label that appears would reflow the row under a
/// thumb that is still on the glass, which is the same reason PendingButton
/// keeps its box.
#[component]
pub fn CopyRow(
    #[prop(into)] label: String,
    #[prop(into)] value: Signal<String>,
    #[prop(optional, into)] testid: String,
) -> impl IntoView {
    let (copied, set_copied) = signal(false);
    let aria = label.clone();

    let on_click = move |_| {
        copy_to_clipboard(&value.get());
        set_copied.set(true);
        // Long enough to read, short enough that a second copy still feels
        // like it did something.
        set_timeout(
            move || set_copied.set(false),
            std::time::Duration::from_millis(1500),
        );
    };

    view! {
        <div class="px-4 py-3 flex items-center gap-3">
            <div class="min-w-0 flex-1">
                <div class="text-fine text-ink-48 mb-1">{label}</div>
                // break-all so a long value wraps instead of widening the
                // column and giving the page a horizontal scrollbar.
                <div data-testid=testid class="text-body break-all">{move || value.get()}</div>
            </div>
            <button
                type="button"
                data-testid="copy"
                aria-label=move || format!("Copy {aria}")
                class="shrink-0 w-11 h-11 inline-flex items-center justify-center rounded-full
                       text-action"
                on:click=on_click
            >
                <span class="material-symbols-outlined text-[20px]" aria-hidden="true">
                    {move || if copied.get() { "check" } else { "content_copy" }}
                </span>
            </button>
            // Announced rather than drawn: the glyph change is invisible to a
            // screen reader, and "Copied" with no visual counterpart would be
            // invisible to everyone else.
            <span class="sr-only" aria-live="polite">
                {move || if copied.get() { "Copied" } else { "" }}
            </span>
        </div>
    }
}

/// Writes to the clipboard, where there is one.
///
/// A no-op on the server, which is not a degraded case: this only ever runs
/// from a click, and a click only exists after hydration.
#[cfg(feature = "hydrate")]
fn copy_to_clipboard(text: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.navigator().clipboard().write_text(text);
    }
}

#[cfg(not(feature = "hydrate"))]
fn copy_to_clipboard(_text: &str) {}
