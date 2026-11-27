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

#[component]
pub fn Landing() -> impl IntoView {
    view! {
        <Title text="Notion calendars"/>

        // Hero on parchment. Display type, negative tracking, and enough air
        // above and below that the sentence is the only thing on screen --
        // which is the whole trick DESIGN.md is describing when it says the
        // chrome recedes.
        <section class="bg-parchment py-20">
            <Column>
                <h1 class="text-[40px] leading-[1.08] tracking-[-0.4px] font-semibold mb-4">
                    "Your Notion pages."
                    <br/>
                    "In your calendar."
                </h1>
                <p class="text-tagline text-ink-48 mb-8">
                    "Edits flow both ways, within a minute."
                </p>
                <ServerLink
                    href="/signin"
                    label="Sign in"
                    testid="signin"
                    button=true
                    class="w-full h-12 inline-flex items-center justify-center rounded-full
                           bg-action text-canvas text-body font-semibold no-underline"
                />
            </Column>
        </section>

        // Dark tile. The alternation is what gives a page rhythm; one
        // uninterrupted parchment column is exactly what made this feel
        // empty.
        <section class="bg-tile-1 text-on-dark py-20">
            <Column>
                <h2 class="text-[28px] leading-[1.14] tracking-[-0.28px] font-semibold mb-6">
                    "Four steps, once."
                </h2>
                <div class="space-y-6">
                    <DarkStep n="1" text="Sign in. You get one CalDAV username and password."/>
                    <DarkStep n="2" text="Connect Notion and choose which pages to share."/>
                    <DarkStep n="3" text="Pick a page. A calendar database is created under it."/>
                    <DarkStep n="4" text="Add the calendar to your phone with the URL shown."/>
                </div>
            </Column>
        </section>

        // Back to canvas: the white tile, for the thing that is actually
        // being sold -- that it works with the calendar app already on the
        // phone.
        <section class="bg-canvas py-20">
            <Column>
                <h2 class="text-[28px] leading-[1.14] tracking-[-0.28px] font-semibold mb-4">
                    "No app to install."
                </h2>
                <p class="text-body text-ink-48 mb-6">
                    "It speaks CalDAV, which iPhone, Mac, Thunderbird and Android clients
                     already speak. You paste a URL into the calendar app you have."
                </p>
                <div class="rounded-lg bg-parchment p-5 text-caption text-ink-48 break-all">
                    "https://caldav.opendiy.vn/calendars/u1/notion-xxxx/"
                </div>
            </Column>
        </section>

        // The honest section. Last, in the quietest surface, because it is
        // for the person who is deciding -- not the person being persuaded.
        <section class="bg-parchment py-20">
            <Column>
                <h2 class="text-[28px] leading-[1.14] tracking-[-0.28px] font-semibold mb-6">
                    "What it will not do."
                </h2>
                <div class="space-y-4 text-body text-ink-48">
                    <p>
                        "It cannot read Notion pages you have not shared with it."
                    </p>
                    <p>
                        "It will not map an existing database. It creates its own, so the
                         columns it depends on are known by id and survive being renamed."
                    </p>
                    <p>
                        "It is a sync, not a backup. A deletion on one side is a deletion
                         on the other."
                    </p>
                    <p>"There is no payment and no subscription."</p>
                </div>

                <div class="mt-12 pt-6 border-t border-hairline flex gap-5 text-caption">
                    <A href="/privacy" attr:class="text-action no-underline">"Privacy"</A>
                    <A href="/terms" attr:class="text-action no-underline">"Terms"</A>
                </div>
            </Column>
        </section>
    }
}

/// A numbered line on a dark tile.
///
/// The number sits in `ink-on-dark` rather than the accent: on a dark surface
/// the accent is reserved for the thing you can press, and nothing here is.
#[component]
fn DarkStep(#[prop(into)] n: String, #[prop(into)] text: String) -> impl IntoView {
    view! {
        <div class="flex gap-4">
            <div class="shrink-0 text-tagline font-semibold text-ink-on-dark w-6">{n}</div>
            <div class="text-body text-on-dark">{text}</div>
        </div>
    }
}

#[component]
pub fn Privacy() -> impl IntoView {
    view! {
        <Prose title="Privacy">
            <Muted>"Last updated: 2026-11-27"</Muted>
            <p>
                "This portal connects a Notion workspace to a CalDAV calendar. Here is
                 everything it stores about you and why."
            </p>

            <Heading text="What it stores"/>
            <p>
                <b>"Account."</b>
                " Your email address and the identifier your sign-in provider issues.
                 Used to recognise you on the next visit, and for nothing else."
            </p>
            <p>
                <b>"Notion access."</b>
                " When you connect a workspace, the access token Notion issues, with the
                 workspace id and name. That token is what lets this read and write the
                 pages you selected -- and only those."
            </p>
            <p>
                <b>"Calendars."</b>
                " For each page you link: the page id, a display name, and the calendar
                 path created for it."
            </p>
            <p>
                <b>"Your CalDAV password, in readable form."</b>
                " This is the part worth reading twice. The portal keeps the password
                 itself, not only a hash of it, because it shows it to you again when you
                 come back. The calendar server separately keeps an argon2 hash, which is
                 the copy that actually checks a login. If you would rather no readable
                 copy existed, do not use this service: there is no setting that changes
                 it."
            </p>

            <Heading text="What it does not store"/>
            <p class="text-ink-48">
                "No card numbers or billing details -- there are no payments. No analytics
                 or advertising trackers. Nothing is sold or shared for marketing."
            </p>

            <Heading text="Where it lives"/>
            <p class="text-ink-48">
                "A private PostgreSQL database, not reachable from the internet. The only
                 third party involved is Notion's own API, which is unavoidable, and the
                 infrastructure the servers run on."
            </p>

            <Heading text="Your controls"/>
            <p class="text-ink-48">
                "Revoke access at any time from Notion's own Connections settings; that
                 invalidates the token held here immediately. To delete the account and
                 everything with it, ask."
            </p>
        </Prose>
    }
}

#[component]
pub fn Terms() -> impl IntoView {
    view! {
        <Prose title="Terms">
            <Muted>"Last updated: 2026-11-27"</Muted>

            <Heading text="The service"/>
            <p>
                "It keeps a Notion database and a CalDAV calendar saying the same thing.
                 It is offered as it is, with no uptime promise."
            </p>

            <Heading text="Price"/>
            <p>
                "There is none. No subscription, no payment, nothing to cancel. If that
                 ever changes, it will not change retroactively for data already here."
            </p>

            <Heading text="Your side"/>
            <p class="text-ink-48">
                "Keep your CalDAV password to yourself -- it reaches every calendar on
                 your account. Do not use this to store anything you cannot afford to
                 lose: it is a sync, not a backup, and a sync faithfully propagates a
                 deletion."
            </p>

            <Heading text="No warranty"/>
            <p class="text-ink-48">
                "No warranty of any kind. Syncing involves another company's API and your
                 own network, and either can fail in ways nothing here can prevent."
            </p>

            <Heading text="Ending it"/>
            <p class="text-ink-48">
                "Disconnect in Notion, or ask for the account to be deleted. Your Notion
                 pages are yours and stay where they are."
            </p>
        </Prose>
    }
}
