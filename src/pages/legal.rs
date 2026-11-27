//! Landing, privacy and terms.
//!
//! Adapted from `notion-caldav-saas`, not copied. Three things in that text
//! are false about this service, and a policy page that misdescribes what a
//! service does is worse than none:
//!
//! - it says CalDAV passwords are stored only as an argon2 hash and shown
//!   once. This portal stores the plaintext on purpose, so it can show it
//!   again. That is a disclosure, so it is disclosed.
//! - it describes Paddle, card handling and a merchant of record. Nothing
//!   here takes payments.
//! - it has a refund policy. There is nothing to refund.
//!
//! What carries across is the structure and the parts that are still true.

use crate::ui::{Column, ServerLink};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;

/// The shell every prose page shares: one column, generous leading, the same
/// type ladder as the rest of the app.
#[component]
fn Prose(#[prop(into)] title: String, children: Children) -> impl IntoView {
    view! {
      <Column class="py-8">
        <Title text=title.clone()/>
        <h1 class="text-title font-semibold mb-6">{title}</h1>
        <div class="text-body space-y-4">{children()}</div>
        <div class="mt-10">
            <A
                href="/"
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

#[component]
fn Heading(#[prop(into)] text: String) -> impl IntoView {
    view! { <h2 class="text-tagline font-semibold mt-8 mb-2">{text}</h2> }
}

#[component]
fn Muted(children: Children) -> impl IntoView {
    view! { <p class="text-ink-48">{children()}</p> }
}

/// Structured data, so a search result shows what this is rather than the
/// first forty words of the page.
///
/// The `description` is notion-caldav-saas's own, minus "free for the first
/// 6 months" -- that one describes a trial before a charge, and there is no
/// charge here. Its `offers` block is dropped for the same reason: claiming
/// a price for something free is a lie a crawler repeats.
const JSON_LD: &str = r##"{
  "@context": "https://schema.org",
  "@type": "SoftwareApplication",
  "name": "NotionCal",
  "url": "https://notion-caldav-portal.opendiy.vn/",
  "applicationCategory": "BusinessApplication",
  "operatingSystem": "Web, iOS, Android",
  "description": "Sync your Notion database with Apple Calendar, Google Calendar, or any CalDAV app. Two-way editing."
}"##;

#[component]
pub fn Landing() -> impl IntoView {
    view! {
        <Title text="NotionCal - Sync Notion with your Calendar"/>
        <script type="application/ld+json" inner_html=JSON_LD></script>

        <section class="bg-parchment pt-20 pb-16">
            <Column>
                // The badge is a pill of text, not a coloured chip: the
                // accent is reserved for what can be pressed.
                <div class="inline-flex items-center gap-2 h-9 px-4 rounded-full
                            bg-canvas border border-hairline text-caption text-ink-48 mb-6">
                    <span class="material-symbols-outlined text-[16px]" aria-hidden="true">"sync"</span>
                    "Two-way sync"
                </div>
                <h1 class="text-[40px] leading-[1.08] tracking-[-0.4px] font-semibold mb-4">
                    "Turn your Notion database into an online calendar"
                </h1>
                <p class="text-tagline text-ink-48 mb-8">
                    "Sync your Notion workspace with Apple Calendar, Google Calendar, or any
                     CalDAV app. Edit freely from either side."
                </p>
                <ServerLink
                    href="/signin"
                    label="Log in / Sign up"
                    testid="signin"
                    button=true
                    class="w-full h-12 inline-flex items-center justify-center rounded-full
                           bg-action text-canvas text-body font-semibold no-underline"
                />
            </Column>
        </section>

        <section class="bg-tile-1 text-on-dark py-20">
            <Column>
                <div class="text-caption text-ink-on-dark mb-2">"A simple process"</div>
                <h2 class="text-[28px] leading-[1.14] tracking-[-0.28px] font-semibold mb-8">
                    "Start syncing your data in just 3 steps"
                </h2>
                <div class="space-y-7">
                    <DarkStep
                        icon="link"
                        title="1. Connect Notion"
                        text="Securely connect your Notion workspace via OAuth."
                    />
                    // Not "Pick a database". This service creates its own
                    // database inside a page you choose, which is the whole
                    // reason /connect exists to say so -- copying the
                    // original sentence here would contradict that screen.
                    <DarkStep
                        icon="description"
                        title="2. Pick a page"
                        text="Choose the page to put the calendar under. A database is created inside it."
                    />
                    <DarkStep
                        icon="event_available"
                        title="3. Sync your calendar"
                        text="Subscribe from your calendar app (Apple, Google, Outlook) via the CalDAV link."
                    />
                </div>
            </Column>
        </section>

        <section class="bg-canvas py-20">
            <Column>
                <div class="space-y-6">
                    <Feature
                        icon="sync_alt"
                        title="Real-time two-way sync"
                        text="Changes in Notion update your calendar instantly, and vice versa. Never miss a deadline."
                    />
                    <Feature
                        icon="security"
                        title="Your data, your control"
                        text="Only reads/writes the pages you authorize. Disconnect anytime."
                    />
                    <Feature
                        icon="devices"
                        title="Works with any calendar app"
                        text="Standard CalDAV protocol."
                    />
                </div>
                <div class="rounded-lg bg-parchment p-4 mt-8 text-caption text-ink-48 break-all">
                    "notion-caldav-portal.opendiy.vn/calendars/..."
                </div>
            </Column>
        </section>

        <section class="bg-parchment py-16">
            <Column>
                <div class="flex gap-5 text-caption">
                    <A href="/privacy" attr:class="text-action no-underline">"Privacy"</A>
                    <A href="/terms" attr:class="text-action no-underline">"Terms"</A>
                </div>
            </Column>
        </section>
    }
}

/// A Material Symbol.
///
/// `aria-hidden`, always: each sits beside text that already says the same
/// thing, so announcing the ligature name would be a second reading of one
/// line.
#[component]
fn Glyph(#[prop(into)] name: String, #[prop(into)] class: String) -> impl IntoView {
    view! {
        <span class=format!("material-symbols-outlined {class}") aria-hidden="true">{name}</span>
    }
}

/// A step on a dark tile. The glyph is `ink-on-dark`, not the accent: on a
/// dark surface the accent means "pressable", and nothing here is.
#[component]
fn DarkStep(
    #[prop(into)] icon: String,
    #[prop(into)] title: String,
    #[prop(into)] text: String,
) -> impl IntoView {
    view! {
        <div class="flex gap-4">
            <Glyph name=icon class="text-ink-on-dark text-[22px] shrink-0 mt-0.5"/>
            <div>
                <div class="text-body font-semibold text-on-dark mb-1">{title}</div>
                <div class="text-body text-ink-on-dark">{text}</div>
            </div>
        </div>
    }
}

#[component]
fn Feature(
    #[prop(into)] icon: String,
    #[prop(into)] title: String,
    #[prop(into)] text: String,
) -> impl IntoView {
    view! {
        <div class="flex gap-4">
            <Glyph name=icon class="text-action text-[22px] shrink-0 mt-0.5"/>
            <div>
                <div class="text-body font-semibold mb-1">{title}</div>
                <div class="text-body text-ink-48">{text}</div>
            </div>
        </div>
    }
}

#[component]
pub fn Privacy() -> impl IntoView {
    view! {
        <Prose title="Privacy Policy">
            <Muted>"Last updated: 2026-11-27"</Muted>
            <p>
                "NotionCal (\"the Service\") turns a Notion database into a CalDAV feed and a
                 browser calendar view. This page explains what data we collect and how we
                 use it."
            </p>

            <Heading text="What we collect"/>
            <p>
                <b>"Account: "</b>
                "the email address from your login (via our self-hosted Keycloak identity
                 provider) — used only to identify your account."
            </p>
            <p>
                <b>"Notion access: "</b>
                "when you connect your Notion workspace, we store the OAuth access token
                 Notion issues us, along with your workspace id/name and integration bot id.
                 This token is what lets the Service read and write the specific pages you
                 granted access to."
            </p>
            <p>
                <b>"Calendar configuration: "</b>
                "for each Notion page you choose to sync, we store its page id, the calendar
                 path created for it, and a display name."
            </p>
            <p>
                // Changed, and the only sentence in this policy that is not
                // the original. It read: "Only a salted hash (Argon2) of the
                // password is stored — the plaintext is shown to you once, at
                // creation time, and never again." That is true of
                // notion-caldav-saas and false here, and a privacy policy
                // that misstates how a credential is stored is the one kind
                // of copy that must not be pasted.
                <b>"CalDAV credentials: "</b>
                "we generate a random username/password for your account so you can subscribe
                 from Apple/Google Calendar or any CalDAV client. We store the password itself,
                 not only a hash, so that it can be shown to you again whenever you return.
                 The calendar server separately stores a salted Argon2 hash, and that is the
                 copy a login is checked against."
            </p>

            <Heading text="What we don't collect"/>
            <p class="text-ink-48">"No card numbers, bank details, or billing addresses — the Service is free and takes no payments."</p>
            <p class="text-ink-48">"No analytics or advertising trackers."</p>
            <p class="text-ink-48">"No data is sold or shared with third parties for marketing."</p>

            <Heading text="How we use it"/>
            <p class="text-ink-48">
                "Solely to operate the Service: fetching events from your Notion database,
                 converting them to CalDAV/iCalendar format, and keeping them in sync via
                 periodic polling."
            </p>

            <Heading text="Where it's stored"/>
            <p class="text-ink-48">
                "All data lives in a private PostgreSQL database we operate, not exposed to
                 the public internet, reachable only by the Service itself. We do not use
                 third-party data processors beyond Notion's own API (needed to read/write
                 your workspace) and the infrastructure hosting our servers."
            </p>

            <Heading text="Your controls"/>
            <p class="text-ink-48">
                "You can revoke the Service's access at any time from Notion's own
                 \"Connections\" settings in your workspace — this immediately invalidates the
                 access token we hold."
            </p>
            <p class="text-ink-48">
                "To delete your account and all associated data (connections, calendars,
                 credentials), email us at the address below."
            </p>

            <Heading text="Contact"/>
            <p class="text-ink-48">"Questions about this policy: huyvu8051@gmail.com"</p>
        </Prose>
    }
}

#[component]
pub fn Terms() -> impl IntoView {
    view! {
        <Prose title="Terms of Service">
            <Muted>"Last updated: 2026-11-27"</Muted>
            <p>"By using NotionCal (\"the Service\"), you agree to these terms."</p>

            <Heading text="The Service"/>
            <p>
                "The Service connects to a Notion workspace you authorize, and exposes the
                 page(s) you choose as a CalDAV feed. There is no guaranteed uptime or support
                 response time."
            </p>

            <Heading text="Price"/>
            // The original reads "Price and billing" and describes a $1/year
            // subscription through Paddle, with a link to a Refund Policy.
            // None of that exists here, and a terms page describing a charge
            // that cannot occur is worse than one that says so plainly.
            <p>
                "The Service is free. There is no subscription, no card, and nothing to
                 cancel."
            </p>

            <Heading text="Your responsibilities"/>
            <p class="text-ink-48">
                "You're responsible for the content of the Notion pages you connect, and for
                 keeping your CalDAV credentials confidential."
            </p>
            <p class="text-ink-48">
                "Don't use the Service to store or distribute illegal content, or in a way
                 that places excessive load on it (e.g. automated scraping outside normal
                 calendar-client sync behavior)."
            </p>
            <p class="text-ink-48">
                "You must have the right to grant the Service access to any Notion workspace
                 you connect."
            </p>

            <Heading text="No warranty"/>
            <p class="text-ink-48">
                "The Service is provided \"as is\" without warranty of any kind. We don't
                 guarantee it will be uninterrupted, error-free, or that data will never be
                 lost — Notion remains the source of truth for your data, and we recommend not
                 relying on the Service as your only backup of important events."
            </p>

            <Heading text="Termination"/>
            <p class="text-ink-48">
                "We may suspend or terminate access to the Service for any account found
                 abusing it (as described above), or discontinue the Service entirely. You may
                 stop using the Service and revoke its Notion access at any time."
            </p>

            <Heading text="Changes"/>
            <p class="text-ink-48">
                "We may update these terms as the Service evolves; continued use after a change
                 means you accept the updated terms."
            </p>

            <Heading text="Governing law"/>
            <p class="text-ink-48">"These terms are governed by the laws of Vietnam."</p>

            <Heading text="Contact"/>
            <p class="text-ink-48">"huyvu8051@gmail.com"</p>
        </Prose>
    }
}
