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
/// No `offers` block: notion-caldav-saas carries one because it charges, and
/// claiming a price for something free would be a lie a crawler repeats.
const JSON_LD: &str = r##"{
  "@context": "https://schema.org",
  "@type": "SoftwareApplication",
  "name": "Notion calendars",
  "url": "https://notion-caldav-portal.opendiy.vn/",
  "applicationCategory": "BusinessApplication",
  "operatingSystem": "Web, iOS, macOS, Android",
  "description": "Sync a Notion page with Apple Calendar, Google Calendar, or any CalDAV app. Edits flow both ways."
}"##;

#[component]
pub fn Landing() -> impl IntoView {
    view! {
        <Title text="Notion calendars"/>
        <script type="application/ld+json" inner_html=JSON_LD></script>

        // Hero on parchment. Display type, negative tracking, and enough air
        // that the sentence is the only thing on screen -- which is the trick
        // DESIGN.md is describing when it says the chrome recedes.
        <section class="bg-parchment pt-20 pb-16">
            <Column>
                <Glyph name="calendar_month" class="text-action text-[32px] mb-5"/>
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
                <p class="text-caption text-ink-48 mt-4 text-center">
                    "Free. No card, no subscription."
                </p>
            </Column>
        </section>

        // Dark tile. The alternation is what gives a page rhythm; one
        // uninterrupted parchment column is what made this feel empty.
        <section class="bg-tile-1 text-on-dark py-20">
            <Column>
                <h2 class="text-[28px] leading-[1.14] tracking-[-0.28px] font-semibold mb-8">
                    "Four steps, once."
                </h2>
                <div class="space-y-7">
                    <DarkStep
                        icon="login" n="1"
                        title="Sign in"
                        text="You get one CalDAV username and password, for every calendar on your account."
                    />
                    <DarkStep
                        icon="link" n="2"
                        title="Connect Notion"
                        text="You choose which pages to share, on Notion's own screen. Nothing else is readable."
                    />
                    <DarkStep
                        icon="database" n="3"
                        title="Pick a page"
                        text="A calendar database is created under it, with the columns already set up."
                    />
                    <DarkStep
                        icon="event_available" n="4"
                        title="Add it to your phone"
                        text="Paste the URL into the calendar app you already use."
                    />
                </div>
            </Column>
        </section>

        // Canvas: the three things worth knowing before signing in.
        <section class="bg-canvas py-20">
            <Column>
                <h2 class="text-[28px] leading-[1.14] tracking-[-0.28px] font-semibold mb-8">
                    "What you get."
                </h2>
                <div class="space-y-5">
                    <Feature
                        icon="sync_alt"
                        title="Both directions"
                        text="Move an event on your phone and the Notion page moves. Edit the page and the
                              event moves. Neither side is a copy of the other."
                    />
                    <Feature
                        icon="devices"
                        title="No app to install"
                        text="It speaks CalDAV, which iPhone, Mac, Thunderbird and Android clients already
                              speak. There is nothing new on your phone."
                    />
                    <Feature
                        icon="security"
                        title="Only what you shared"
                        text="It reads and writes the pages you picked on Notion, and nothing else. Revoke
                              it from Notion at any time."
                    />
                </div>

                <div class="rounded-lg bg-parchment p-4 mt-8">
                    <div class="text-fine text-ink-48 mb-1">"The URL you paste"</div>
                    <div class="text-caption break-all">
                        "https://caldav.opendiy.vn/calendars/u1/notion-xxxx/"
                    </div>
                </div>
            </Column>
        </section>

        // The honest section. Last, in the quietest surface, because it is
        // for the person deciding -- not the person being persuaded.
        <section class="bg-parchment py-20">
            <Column>
                <h2 class="text-[28px] leading-[1.14] tracking-[-0.28px] font-semibold mb-6">
                    "What it will not do."
                </h2>
                <div class="space-y-4 text-body text-ink-48">
                    <p>"It cannot read Notion pages you have not shared with it."</p>
                    <p>
                        "It will not map an existing database. It creates its own, so the
                         columns it depends on are known by id and survive being renamed."
                    </p>
                    <p>
                        "It is a sync, not a backup. A deletion on one side is a deletion
                         on the other."
                    </p>
                    <p>
                        "It keeps your CalDAV password in readable form, so it can show it
                         to you again. The "
                        <A href="/privacy" attr:class="text-action no-underline">"privacy page"</A>
                        " says exactly what that means."
                    </p>
                </div>

                <div class="mt-10">
                    <ServerLink
                        href="/signin"
                        label="Sign in"
                        testid="signin-bottom"
                        button=true
                        class="w-full h-12 inline-flex items-center justify-center rounded-full
                               bg-action text-canvas text-body font-semibold no-underline"
                    />
                </div>

                <div class="mt-10 pt-6 border-t border-hairline flex gap-5 text-caption">
                    <A href="/privacy" attr:class="text-action no-underline">"Privacy"</A>
                    <A href="/terms" attr:class="text-action no-underline">"Terms"</A>
                </div>
            </Column>
        </section>
    }
}

/// A Material Symbol.
///
/// `aria-hidden`, always: every one of these sits beside text that already
/// says the same thing, so announcing the ligature name would be a second
/// reading of the same line.
#[component]
fn Glyph(#[prop(into)] name: String, #[prop(into)] class: String) -> impl IntoView {
    view! {
        <span class=format!("material-symbols-outlined {class}") aria-hidden="true">{name}</span>
    }
}

/// A numbered step on a dark tile.
///
/// The number and glyph sit in `ink-on-dark`, not the accent: on a dark
/// surface the accent is reserved for the thing you can press, and nothing
/// here is pressable.
#[component]
fn DarkStep(
    #[prop(into)] icon: String,
    #[prop(into)] n: String,
    #[prop(into)] title: String,
    #[prop(into)] text: String,
) -> impl IntoView {
    view! {
        <div class="flex gap-4">
            <Glyph name=icon class="text-ink-on-dark text-[22px] shrink-0 mt-0.5"/>
            <div>
                <div class="text-body font-semibold text-on-dark mb-1">
                    {n}". "{title}
                </div>
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
