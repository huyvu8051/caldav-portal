#![recursion_limit = "1024"]

//! Account portal for `caldav-rs` + `notion-bridge`.
//!
//! Sign up, sign in, connect Notion, link a database to a calendar, read the
//! CalDAV credentials back. It owns accounts and links; it does not own
//! calendar data and never reaches into another service's tables -- the same
//! boundary `notion-bridge` keeps, for the same reason.

#[cfg(feature = "ssr")]
pub mod caldav_admin;
#[cfg(feature = "ssr")]
pub mod caldav_client;
#[cfg(feature = "ssr")]
pub mod db;
#[cfg(feature = "ssr")]
pub mod notion_api;
#[cfg(feature = "ssr")]
pub mod notion_oauth;
#[cfg(feature = "ssr")]
pub mod oidc;
pub mod pages;
#[cfg(feature = "ssr")]
pub mod routes;
#[cfg(feature = "ssr")]
pub mod state;
pub mod ui;

use leptos::prelude::*;
use leptos_meta::{MetaTags, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::StaticSegment;

#[cfg(feature = "ssr")]
pub fn init_executor() {
    any_spawner::Executor::init_tokio().expect("failed to init leptos reactive executor");
}

// Tokens from DESIGN.md, carried across from webcaldav's `src/lib.rs`
// unchanged. The point of a design system is that two apps in the same family
// look like one thing, and `e2e/tests/design-system.spec.ts` asserts that on
// computed styles -- so drift between the two is a failing test, not a matter
// of taste.
//
// `maxWidth.app` stays at 430px. The UI is phone-shaped and stays
// phone-shaped: on a desktop the same column is centred and the parchment
// background expands around it. A second, wider layout would be a second
// layout to keep in step with the first, and every page added later would
// have to be designed and tested twice.
const TAILWIND_CONFIG: &str = r##"
tailwind.config = {
  theme: {
    extend: {
      colors: {
        // "The single accent is non-negotiable" -- every interactive element
        // is Action Blue and nothing else.
        "action": "#0066cc",
        "action-focus": "#0071e3",
        // Text
        "ink": "#1d1d1f",
        "ink-80": "#333333",
        // #6e6e73, not DESIGN.md's #7a7a7a: webcaldav measured the latter at
        // 3.94:1 on parchment, under AA's 4.5 for text at this size, and
        // swapped in Apple's own secondary-label grey. Carried across rather
        // than rediscovered the hard way.
        "ink-48": "#6e6e73",
        // Surfaces
        "canvas": "#ffffff",
        "parchment": "#f5f5f7",
        "pearl": "#fafafc",
        // Hairlines -- the only separator in the system
        "hairline": "#e0e0e0",
        "divider-soft": "#f0f0f0",
        // Full-bleed tiles. webcaldav leaves these out on purpose -- it is
        // dense UI, not a landing page -- but a landing page is exactly what
        // DESIGN.md derived them from.
        "tile-1": "#272729",
        "tile-2": "#2a2a2c",
        "on-dark": "#ffffff",
        "ink-on-dark": "#cccccc",
        // The accent, on a dark tile. A different value, not a second
        // accent: 209 degrees against Action Blue's 210, which is why the
        // one-hue rule still holds.
        "action-on-dark": "#2997ff",
      },
      fontFamily: {
        // Resolves to SF Pro on Apple platforms and a sane native face
        // elsewhere. No webfont download for text.
        "sans": ["-apple-system", "BlinkMacSystemFont", "'SF Pro Text'",
                 "'SF Pro Display'", "system-ui", "'Segoe UI'", "Roboto",
                 "sans-serif"],
      },
      fontSize: {
        // "Run body copy at 17px -- not 16px. The extra pixel defines the
        // brand's reading pace." Tracking is negative throughout.
        "fine":    ["12px", { lineHeight: "1.33", letterSpacing: "-0.12px" }],
        "caption": ["14px", { lineHeight: "1.43", letterSpacing: "-0.224px" }],
        "body":    ["17px", { lineHeight: "1.47", letterSpacing: "-0.374px" }],
        "tagline": ["21px", { lineHeight: "1.38", letterSpacing: "-0.374px" }],
        "title":   ["28px", { lineHeight: "1.14", letterSpacing: "-0.28px" }],
      },
      fontWeight: {
        // 500 is absent on purpose: `font-medium` fails to resolve, which is
        // how a stray one gets caught before the test has to.
        "light": "300",
        "normal": "400",
        "semibold": "600",
        "bold": "700",
      },
      borderRadius: {
        "sm": "8px",      // compact utility
        "md": "11px",     // pearl capsule
        "lg": "18px",     // cards
        "full": "9999px", // the signature pill -- actions only
      },
      maxWidth: {
        // iPhone 15 Pro Max logical width. See the note above.
        "app": "430px",
      },
    },
  },
};
"##;

// No shadows anywhere: DESIGN.md allows exactly one and reserves it for
// product photography, of which there is none here. Depth is surface-colour
// change plus hairlines.
const BASE_STYLE: &str = r#"
html, body { margin: 0; padding: 0; background-color: #f5f5f7; }
body { -webkit-font-smoothing: antialiased; -moz-osx-font-smoothing: grayscale; }

/* scale(0.95) as the press state on every button -- the system-wide
   micro-interaction. Applied globally so no button can be born without it. */
button:active, a[role="button"]:active { transform: scale(0.95); }
button, a[role="button"] { transition: transform 120ms ease, background-color 120ms ease, color 120ms ease; }
/* Without this Safari keeps double-tap-to-zoom live on every control and
   waits to see whether a second tap is coming before dispatching the click.
   A tap that lands ~300ms late is indistinguishable from a slow app. */
button, a, [role="button"], input, select, textarea, label { touch-action: manipulation; }

/* The native select arrow sits hard against the right edge, so on a pill it
   reads as falling outside the curve, and each control's width puts it
   somewhere different. One chevron, one 16px inset, in Action Blue because
   it is the affordance. */
select.field-select {
  -webkit-appearance: none;
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='8' viewBox='0 0 12 8'%3E%3Cpath d='M1 1.5 6 6.5 11 1.5' fill='none' stroke='%230066cc' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 1rem center;
  background-size: 12px 8px;
  padding-right: 2.5rem;
}
/* Respect the platform preference rather than animating regardless. */
@media (prefers-reduced-motion: reduce) {
  button:active, a[role="button"]:active { transform: none; }
  * { transition-duration: 0.01ms !important; animation-duration: 0.01ms !important; }
}

/* Visible only to a screen reader. Tailwind ships this, but the CDN build
   is configured from `tailwind.config` above and nothing here guarantees the
   utility survives a future change -- and a live region that silently stops
   being announced is not something a test would catch. */
.sr-only {
  position: absolute;
  width: 1px; height: 1px;
  padding: 0; margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border-width: 0;
}

@keyframes spin { to { transform: rotate(360deg); } }
/* Indeterminate activity indicator (ui.rs). Sized in em and painted in
   currentColor so it belongs to whatever control it sits in. */
.spinner {
  display: inline-block;
  width: 1em;
  height: 1em;
  border: 2px solid currentColor;
  border-top-color: transparent;
  border-radius: 9999px;
  animation: spin 0.7s linear infinite;
  flex: none;
}
/* The blanket rule above sets every animation to 0.01ms, which would strobe
   this rather than calm it. A still ring still reads as busy. */
@media (prefers-reduced-motion: reduce) {
  .spinner { animation: none !important; opacity: 0.55; }
}
"#;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"/>
                <meta name="theme-color" content="#f5f5f7"/>
                // Material Symbols is the only font request: the text stack is
                // the system one, and the UI uses glyphs.
                <link rel="preconnect" href="https://fonts.googleapis.com"/>
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin=""/>
                <link href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@24,400,0,0&display=swap" rel="stylesheet"/>
                <script src="https://cdn.tailwindcss.com"></script>
                <script>{TAILWIND_CONFIG}</script>
                <style>{BASE_STYLE}</style>
                <AutoReload options=options.clone()/>
                <HydrationScripts options=options.clone()/>
                <MetaTags/>
            </head>
            <body class="font-sans text-body text-ink bg-parchment">
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    leptos_meta::provide_meta_context();
    view! {
        // A per-page <Title/> overrides this; it is the fallback for any
        // route that does not set one, and the name in a bookmark.
        <Title text="Notion calendars"/>
        <Router>
            // No column here. It used to be `mx-auto max-w-app px-5 py-8`,
            // which every page inherited and no page could escape -- and a
            // landing page whose sections stop at 430px of white is the
            // definition of looking empty. Each page applies `ui::Column`
            // itself; the landing applies it inside each full-bleed section.
            <main>
                <Routes fallback=|| view! { <pages::not_found::NotFound/> }>
                    <Route path=StaticSegment("") view=pages::legal::Landing/>
                    <Route path=StaticSegment("privacy") view=pages::legal::Privacy/>
                    <Route path=StaticSegment("terms") view=pages::legal::Terms/>
                    <Route path=StaticSegment("me") view=pages::me::Me/>
                </Routes>
            </main>
        </Router>
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
