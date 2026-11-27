//! The screen before Notion's screen.
//!
//! Notion's "Select pages" dialog lists pages *and* databases, and nothing in
//! the OAuth authorize URL can filter it -- `client_id`, `response_type`,
//! `owner`, `redirect_uri`, `state` is the whole vocabulary. notion-caldav-saas
//! sends exactly the same five, and has the same problem.
//!
//! So the only place this can be fixed is before the redirect. Which is worth
//! doing properly rather than with a line of small print: picking a database
//! here is not a small mistake, it is the one choice that leads to an empty
//! picker on the next page with no explanation.

use crate::ui::{Column, ServerLink};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;

#[component]
pub fn Connect() -> impl IntoView {
    view! {
      <Column class="py-8">
        <Title text="Connect Notion"/>
        <h1 class="text-title font-semibold mb-2">"Pick a page, not a database"</h1>
        <p class="text-body text-ink-48 mb-6">
            "Notion's next screen lists both. Only a page works here."
        </p>

        <div class="rounded-lg bg-canvas border border-hairline divide-y divide-divider-soft mb-6">
            <Choice
                icon="description"
                ok=true
                title="A page"
                text="The calendar database is created inside it. Any page will do -- an empty one is fine."
            />
            <Choice
                icon="database"
                ok=false
                title="A database"
                text="Sharing one does nothing. This creates its own database, so the columns it depends on are known by id and cannot be renamed away."
            />
        </div>

        <ServerLink
            href="/notion/connect"
            label="Continue to Notion"
            testid="continue-to-notion"
            button=true
            class="w-full h-12 inline-flex items-center justify-center rounded-full
                   bg-action text-canvas text-body font-semibold no-underline"
        />

        <div class="mt-4">
            <A
                href="/me"
                attr:role="button"
                attr:class="w-full h-12 inline-flex items-center justify-center rounded-full
                            bg-canvas border border-hairline text-action text-body no-underline"
            >
                "Back"
            </A>
        </div>
      </Column>
    }
}

/// One row of the do/do-not pair.
///
/// The mark is a glyph plus the words "Use this" / "Not used", never colour
/// alone: red and green would be a second and third accent, and would say
/// nothing to anyone who cannot tell them apart.
#[component]
fn Choice(
    #[prop(into)] icon: String,
    ok: bool,
    #[prop(into)] title: String,
    #[prop(into)] text: String,
) -> impl IntoView {
    view! {
        <div class="px-4 py-4 flex gap-3">
            <span
                class=if ok { "material-symbols-outlined text-action text-[22px] shrink-0 mt-0.5" }
                      else { "material-symbols-outlined text-ink-48 text-[22px] shrink-0 mt-0.5" }
                aria-hidden="true"
            >
                {icon}
            </span>
            <div>
                <div class="text-body font-semibold mb-1">
                    {title}
                    <span class=if ok { "text-caption text-action ml-2" }
                                else { "text-caption text-ink-48 ml-2" }>
                        {if ok { "Use this" } else { "Not used" }}
                    </span>
                </div>
                <div class="text-body text-ink-48">{text}</div>
            </div>
        </div>
    }
}
