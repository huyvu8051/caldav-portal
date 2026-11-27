//! The credentials page: what a person came here to get.

use crate::ui::{Column, PendingButton, ServerLink};
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    /// `true` when Notion has refused this token. Shown rather than hidden:
    /// a calendar that silently stopped updating is the failure this whole
    /// field exists to prevent.
    pub needs_reconnect: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub email: String,
    pub server: String,
    pub username: String,
    pub password: String,
    pub workspaces: Vec<Workspace>,
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
            workspaces: workspaces(&server, account_id).await?,
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
            workspaces: workspaces(&server, account_id).await?,
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
        workspaces: workspaces(&server, account_id).await?,
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
        })
        .collect())
}

/// The Notion workspaces this account has connected.
#[cfg(feature = "ssr")]
async fn workspaces(
    server: &crate::state::ServerState,
    account_id: i64,
) -> Result<Vec<Workspace>, ServerFnError> {
    let rows = server
        .db
        .notion_connections(account_id)
        .await
        .map_err(|e| ServerFnError::new(format!("could not list connections: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|c| Workspace {
            id: c.id,
            // Notion omits the name for some grants; the id is never a good
            // thing to show a person, but it beats an empty row.
            name: if c.workspace_name.is_empty() {
                c.workspace_id
            } else {
                c.workspace_name
            },
            needs_reconnect: c.token_invalid_since.is_some(),
        })
        .collect())
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
        workspaces: workspaces(&server, account_id).await?,
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
            <Row label="Server" value=Signal::derive(move || credentials.get().server) testid="server"/>
            <Row label="User Name" value=Signal::derive(move || credentials.get().username) testid="username"/>
            <Row label="Password" value=Signal::derive(move || credentials.get().password) testid="password"/>
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

        <h2 class="text-tagline font-semibold mt-10 mb-2">"Calendars"</h2>
        <LinkList links=Signal::derive(move || credentials.get().links)
                  base=Signal::derive(move || credentials.get().server)/>

        <h2 class="text-tagline font-semibold mt-10 mb-2">"Notion"</h2>
        <NotionSection workspaces=Signal::derive(move || credentials.get().workspaces)/>

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
fn Row(
    #[prop(into)] label: String,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] testid: String,
) -> impl IntoView {
    view! {
        <div class="px-4 py-3">
            <div class="text-fine text-ink-48 mb-1">{label}</div>
            // break-all so a long value wraps instead of widening the column
            // and giving the whole page a horizontal scrollbar on a phone.
            <div data-testid=testid class="text-body break-all">{move || value.get()}</div>
        </div>
    }
}

#[component]
fn NotionSection(#[prop(into)] workspaces: Signal<Vec<Workspace>>) -> impl IntoView {
    view! {
        <Show
            when=move || !workspaces.get().is_empty()
            fallback=|| {
                view! {
                    <p class="text-body text-ink-48 mb-4">
                        "No workspace connected yet."
                    </p>
                    <ConnectButton label="Connect Notion"/>
                }
            }
        >
            <div class="rounded-lg bg-canvas border border-hairline divide-y divide-divider-soft mb-4">
                <For
                    each=move || workspaces.get()
                    key=|w| (w.id, w.needs_reconnect)
                    let:w
                >
                    <div class="px-4 py-3">
                        <div data-testid="workspace" class="text-body break-all">{w.name.clone()}</div>
                        <Show when=move || w.needs_reconnect>
                            // Said plainly, not as a warning icon: the whole
                            // point is that the person finds out here rather
                            // than from a calendar that quietly stopped.
                            <div data-testid="needs-reconnect" class="text-fine text-ink-48 mt-1">
                                "Notion refused this connection. Reconnect to resume syncing."
                            </div>
                        </Show>
                    </div>
                </For>
            </div>
            <ConnectButton label="Connect another workspace"/>
        </Show>
    }
}

#[component]
fn ConnectButton(#[prop(into)] label: String) -> impl IntoView {
    // A link, not a button with an Action: the flow is a full-page redirect
    // to Notion, so there is no in-flight state for a spinner to describe.
    view! {
        <ServerLink
            href="/notion/connect"
            label=label
            testid="connect-notion"
            button=true
            class="w-full h-12 inline-flex items-center justify-center rounded-full
                   bg-canvas border border-hairline text-action text-body no-underline"
        />
    }
}

#[component]
fn LinkList(
    #[prop(into)] links: Signal<Vec<CalendarLink>>,
    /// The CalDAV origin, so each row can show a URL rather than a path. A
    /// path is not something anyone can paste into a calendar client.
    #[prop(into)]
    base: Signal<String>,
) -> impl IntoView {
    view! {
        <Show
            when=move || !links.get().is_empty()
            fallback=|| view! {
                <p class="text-body text-ink-48 mb-4">"No calendars yet."</p>
            }
        >
            <div class="rounded-lg bg-canvas border border-hairline divide-y divide-divider-soft mb-4">
                <For each=move || links.get() key=|l| l.id let:l>
                    <div class="px-4 py-3">
                        <div data-testid="link-name" class="text-body break-all">{l.name.clone()}</div>
                        // The full URL, for clients that cannot walk
                        // /.well-known/caldav to the calendar home and have
                        // to be handed the collection itself.
                        <div data-testid="link-url" class="text-fine text-ink-48 break-all mt-1">
                            {
                                let uri = l.uri.clone();
                                move || format!("{}{}", base.get().trim_end_matches('/'), uri)
                            }
                        </div>
                    </div>
                </For>
            </div>
        </Show>
        <A
            href="/add"
            attr:data-testid="add-calendar"
            attr:role="button"
            attr:class="w-full h-12 inline-flex items-center justify-center rounded-full
                        bg-canvas border border-hairline text-action text-body no-underline"
        >
            "Add a calendar"
        </A>
    }
}
