//! The credentials page: what a person came here to get.

use crate::ui::{Column, CopyRow, PendingButton, ServerLink};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarLink {
    pub id: i64,
    pub name: String,
    /// The collection path. Joined with the server origin for display,
    /// because a client that cannot discover needs the whole URL.
    pub uri: String,
    /// Where this calendar lives in Notion. Empty when the link predates
    /// template duplication and has no database recorded.
    pub notion_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub email: String,
    pub server: String,
    pub username: String,
    pub password: String,
    /// `true` when Notion has refused a connection, so the calendars under
    /// it have stopped syncing.
    ///
    /// All that survives of the workspace list: a name nobody can act on was
    /// three queries a page load, and this is the one fact that is worth
    /// interrupting someone for. A calendar that quietly stops updating is
    /// the failure this whole product is arranged against; saying nothing
    /// here is how it would win.
    pub notion_broken: bool,
    pub links: Vec<CalendarLink>,
}

/// Everything the server knows about the signed-in person, provisioning the
/// CalDAV credential if this is the first visit.
///
/// Provisioning here rather than in the callback is deliberate. The callback
/// has a browser waiting on a redirect, and argon2 takes ~17ms on the other
/// side of a network call; more importantly, a failure there would strand
/// someone mid-login with no page to explain it. Here it is just this page
/// failing to load, with a retry one tap away.
#[server]
pub async fn my_credentials() -> Result<Option<Credentials>, ServerFnError> {
    let Some((server, account_id)) = session().await? else {
        return Ok(None);
    };

    let account = server
        .db
        .account(account_id)
        .await
        .map_err(|e| ServerFnError::new(format!("account lookup failed: {e}")))?
        .ok_or_else(|| ServerFnError::new("session points at no account"))?;

    if let (Some(u), Some(p)) = (&account.caldav_username, &account.caldav_password) {
        return Ok(Some(Credentials {
            email: account.email,
            server: server.caldav_public_url.clone(),
            username: u.clone(),
            password: p.clone(),
            notion_broken: notion_broken(&server, account_id).await?,
            links: links(&server, account_id).await?,
        }));
    }

    // Short, because it gets typed into a phone by hand. The account id is
    // not a secret -- the password is what protects the account, and it is
    // ~99 bits behind argon2.
    let username = format!("u{account_id}");
    let password = crate::oidc::readable_password();

    // The database claims first. `claim_credential` only writes where the
    // column is still NULL, so if two tabs race, exactly one proceeds to call
    // caldav-rs; the loser re-reads and shows what the winner stored. Calling
    // caldav-rs first and recording afterwards would leave the loser's
    // password in the portal and the winner's hash in caldav-rs.
    let claimed = server
        .db
        .claim_credential(account_id, &username, &password)
        .await
        .map_err(|e| ServerFnError::new(format!("could not claim a credential: {e}")))?;

    if !claimed {
        let again = server
            .db
            .account(account_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .ok_or_else(|| ServerFnError::new("account vanished"))?;
        return Ok(Some(Credentials {
            email: again.email,
            server: server.caldav_public_url.clone(),
            username: again.caldav_username.unwrap_or_default(),
            password: again.caldav_password.unwrap_or_default(),
            notion_broken: notion_broken(&server, account_id).await?,
            links: links(&server, account_id).await?,
        }));
    }

    match server
        .caldav
        .create_principal(&username, &password, &account.email)
        .await
    {
        Ok(()) => {}
        // The principal exists but this portal had not recorded it: a
        // half-finished earlier attempt. Rotating to the password just
        // claimed makes the two sides agree again rather than leaving a
        // credential nobody knows.
        Err(crate::caldav_admin::Error::Conflict) => {
            server
                .caldav
                .set_password(&username, &password)
                .await
                .map_err(|e| ServerFnError::new(format!("could not reconcile: {e}")))?;
        }
        Err(e) => return Err(ServerFnError::new(format!("provisioning failed: {e}"))),
    }

    Ok(Some(Credentials {
        email: account.email,
        server: server.caldav_public_url.clone(),
        username,
        password,
        notion_broken: notion_broken(&server, account_id).await?,
        links: links(&server, account_id).await?,
    }))
}

/// The calendars this account has linked.
#[cfg(feature = "ssr")]
async fn links(
    server: &crate::state::ServerState,
    account_id: i64,
) -> Result<Vec<CalendarLink>, ServerFnError> {
    let rows = server
        .db
        .links(account_id)
        .await
        .map_err(|e| ServerFnError::new(format!("could not list links: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|l| CalendarLink {
            id: l.id,
            name: l.display_name,
            uri: l.calendar_uri,
            // notion.so accepts the id with the dashes stripped, which is
            // the form Notion's own links use.
            notion_url: l
                .notion_database_id
                .map(|id| format!("https://notion.so/{}", id.replace('-', "")))
                .unwrap_or_default(),
        })
        .collect())
}

/// Whether any Notion connection has been refused.
#[cfg(feature = "ssr")]
async fn notion_broken(
    server: &crate::state::ServerState,
    account_id: i64,
) -> Result<bool, ServerFnError> {
    Ok(server
        .db
        .notion_connections(account_id)
        .await
        .map_err(|e| ServerFnError::new(format!("could not list connections: {e}")))?
        .iter()
        .any(|c| c.token_invalid_since.is_some()))
}

/// Issue a new password, in caldav-rs first.
///
/// Order matters and is the opposite of provisioning. Writing here first and
/// failing there would leave the portal displaying a password that cannot log
/// in -- the exact failure the whole two-copy arrangement has to avoid. This
/// way a failure leaves both sides on the old password, which still works.
#[server]
pub async fn rotate_password() -> Result<Credentials, ServerFnError> {
    let Some((server, account_id)) = session().await? else {
        return Err(ServerFnError::new("not signed in"));
    };
    let account = server
        .db
        .account(account_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("session points at no account"))?;
    let username = account
        .caldav_username
        .ok_or_else(|| ServerFnError::new("no credential to rotate yet"))?;

    let password = crate::oidc::readable_password();
    server
        .caldav
        .set_password(&username, &password)
        .await
        .map_err(|e| ServerFnError::new(format!("rotation refused: {e}")))?;
    server
        .db
        .set_password(account_id, &password)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(Credentials {
        email: account.email,
        server: server.caldav_public_url.clone(),
        username,
        password,
        notion_broken: notion_broken(&server, account_id).await?,
        links: links(&server, account_id).await?,
    })
}

/// The session cookie, resolved to an account. `None` means signed out, which
/// is a state to render rather than an error.
#[cfg(feature = "ssr")]
pub(crate) async fn session() -> Result<Option<(crate::state::ServerState, i64)>, ServerFnError> {
    use axum::extract::Extension;
    use axum_extra::extract::cookie::CookieJar;

    let Extension(server) = leptos_axum::extract::<Extension<crate::state::ServerState>>().await?;
    let jar = leptos_axum::extract::<CookieJar>().await?;
    let Some(cookie) = jar.get(crate::routes::SESSION_COOKIE) else {
        return Ok(None);
    };
    match server.db.session_account(cookie.value()).await {
        Ok(Some(id)) => Ok(Some((server, id))),
        Ok(None) => Ok(None),
        Err(e) => Err(ServerFnError::new(format!("session lookup failed: {e}"))),
    }
}

#[component]
pub fn Me() -> impl IntoView {
    let credentials = Resource::new(|| (), |_| my_credentials());
    let rotate = Action::new(|(): &()| async { rotate_password().await });
    let rotated = rotate.value();

    view! {
      <Column class="py-8">
        <Suspense fallback=|| {
            view! { <p class="text-body text-ink-48">"Loading…"</p> }
        }>
            {move || Suspend::new(async move {
                match credentials.await {
                    Err(e) => view! { <ErrorBox message=e.to_string()/> }.into_any(),
                    Ok(None) => view! { <SignedOut/> }.into_any(),
                    Ok(Some(c)) => {
                        let shown = Signal::derive(move || {
                            rotated.get().and_then(|r| r.ok()).unwrap_or_else(|| c.clone())
                        });
                        view! { <CredentialCard credentials=shown rotate=rotate/> }.into_any()
                    }
                }
            })}
        </Suspense>
      </Column>
    }
}

#[component]
fn SignedOut() -> impl IntoView {
    view! {
        <h1 class="text-title font-semibold mb-2">"Your calendars"</h1>
        <p class="text-body text-ink-48 mb-6">
            "Sign in to get the CalDAV details for your phone."
        </p>
        <ServerLink
            href="/signin"
            label="Sign in"
            testid="signin"
            button=true
            class="w-full h-12 inline-flex items-center justify-center rounded-full
                   bg-action text-canvas text-body font-semibold no-underline"
        />
    }
}

#[component]
fn ErrorBox(message: String) -> impl IntoView {
    view! {
        <h1 class="text-title font-semibold mb-2">"Something went wrong"</h1>
        <p data-testid="error" class="text-body text-ink-48 mb-6">{message}</p>
        <A
            href="/me"
            attr:role="button"
            attr:class="w-full h-12 inline-flex items-center justify-center rounded-full
                        bg-canvas border border-hairline text-action text-body no-underline"
        >
            "Try again"
        </A>
    }
}

#[component]
fn CredentialCard(
    #[prop(into)] credentials: Signal<Credentials>,
    rotate: Action<(), Result<Credentials, ServerFnError>>,
) -> impl IntoView {
    view! {
        <Title text="Your account"/>
        <h1 class="text-title font-semibold mb-2">"Your CalDAV account"</h1>
        <p class="text-body text-ink-48 mb-6">
            "On iPhone: Settings → Calendar → Accounts → Add Account → Other → Add CalDAV Account."
        </p>

        <div class="rounded-lg bg-canvas border border-hairline divide-y divide-divider-soft">
            <CopyRow label="Server" value=Signal::derive(move || credentials.get().server) testid="server"/>
            <CopyRow label="User Name" value=Signal::derive(move || credentials.get().username) testid="username"/>
            <CopyRow label="Password" value=Signal::derive(move || credentials.get().password) testid="password"/>
        </div>

        <div class="mt-6">
            <PendingButton
                pending=rotate.pending()
                label="New password"
                busy="Changing"
                testid="rotate"
                class="w-full h-12 inline-flex items-center justify-center gap-2
                       rounded-full bg-canvas border border-hairline text-action
                       text-body disabled:opacity-40 disabled:cursor-not-allowed"
                on_click=Callback::new(move |()| {
                    rotate.dispatch(());
                })
            />
            <p class="mt-2 text-fine text-ink-48">
                "Changing it signs out any device using the old one."
            </p>
        </div>

        <Show when=move || credentials.get().notion_broken>
            <div data-testid="needs-reconnect"
                 class="rounded-lg bg-canvas border border-hairline p-4 mt-10">
                <p class="text-body mb-3">
                    "Notion refused a connection. Calendars under it have stopped syncing."
                </p>
                <ServerLink
                    href="/notion/connect"
                    label="Reconnect Notion"
                    testid="reconnect-notion"
                    button=true
                    class="w-full h-11 inline-flex items-center justify-center rounded-full
                           bg-action text-canvas text-body font-semibold no-underline"
                />
            </div>
        </Show>

        <h2 class="text-tagline font-semibold mt-10 mb-2">"Calendars"</h2>
        <LinkList
            links=Signal::derive(move || credentials.get().links)
            base=Signal::derive(move || credentials.get().server)
            // Unused distinction now: connecting *is* adding. Kept as a
            // signal only so the empty-state wording can differ.
            // Only changes the empty-state wording: connecting is adding,
            // so the button goes to the same place either way.
            needs_notion=Signal::derive(move || credentials.get().links.is_empty())
        />


        <div class="mt-8">
            <ServerLink
                href="/signout"
                label="Sign out"
                testid="signout"
                class="text-body text-action no-underline inline-flex items-center h-11"
            />
        </div>
    }
}

#[component]
fn LinkList(
    #[prop(into)] links: Signal<Vec<CalendarLink>>,
    /// The CalDAV origin, so each row can show a URL rather than a path. A
    /// path is not something anyone can paste into a calendar client.
    #[prop(into)]
    base: Signal<String>,
    /// `true` when no healthy Notion connection exists, so the one button on
    /// this page has to go through connecting first.
    #[prop(into)]
    needs_notion: Signal<bool>,
) -> impl IntoView {
    view! {
        <Show
            when=move || !links.get().is_empty()
            fallback=move || view! {
                <p class="text-body text-ink-48 mb-4">
                    {move || if needs_notion.get() {
                        "No calendars yet. Connect Notion to get started."
                    } else {
                        "No calendars yet. Add one and Notion will create the database."
                    }}
                </p>
            }
        >
            <div class="rounded-lg bg-canvas border border-hairline divide-y divide-divider-soft mb-4">
                <For each=move || links.get() key=|l| l.id let:l>
                    {
                        // The full URL, for clients that cannot walk
                        // /.well-known/caldav to the calendar home and have
                        // to be handed the collection itself. Which is
                        // exactly the kind of thing nobody retypes by hand.
                        let uri = l.uri.clone();
                        let url = Signal::derive(move || {
                            format!("{}{}", base.get().trim_end_matches('/'), uri)
                        });
                        let notion = l.notion_url.clone();
                        let has_notion = !notion.is_empty();
                        view! {
                            <div>
                                <CopyRow label=l.name.clone() value=url testid="link-url"/>
                                <Show when=move || has_notion>
                                    // Opens in Notion. `rel="external"` is not
                                    // needed -- the router only intercepts
                                    // same-origin anchors -- but target and rel
                                    // are: without `noopener` the opened page
                                    // gets a handle back to this one.
                                    <a
                                        href=notion.clone()
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        data-testid="open-in-notion"
                                        class="px-4 pb-3 -mt-1 inline-flex items-center gap-1
                                               text-caption text-action no-underline h-11"
                                    >
                                        <span class="material-symbols-outlined text-[16px]"
                                              aria-hidden="true">"open_in_new"</span>
                                        "Open in Notion"
                                    </a>
                                </Show>
                            </div>
                        }
                    }
                </For>
            </div>
        </Show>
        // Straight to Notion. There is no page to pick and no screen in
        // between: Notion duplicates the calendar database during consent,
        // and the calendar exists by the time the browser comes back.
        <ServerLink
            href="/notion/connect"
            label="Add a calendar"
            testid="add-calendar"
            button=true
            class="w-full h-12 inline-flex items-center justify-center rounded-full
                   bg-action text-canvas text-body font-semibold no-underline"
        />
    }
}
