//! Picking a Notion page to put a calendar under, and the link that makes.

use crate::ui::{Column, PendingButton, ServerLink};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotionPage {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Choices {
    pub connection_id: i64,
    pub workspace: String,
    pub pages: Vec<NotionPage>,
    /// Databases this grant can see. Only meaningful when `pages` is empty:
    /// it is the difference between "you shared nothing" and "you shared the
    /// wrong kind of thing".
    pub databases: usize,
}

/// Pages the user could put a calendar under, for their first healthy
/// connection.
///
/// A revoked token is reported as such rather than as an empty list: an empty
/// picker and a dead connection look identical on screen, and only one of
/// them has a fix.
#[server]
pub async fn notion_choices() -> Result<Option<Choices>, ServerFnError> {
    let Some((server, account_id)) = crate::pages::me::session().await? else {
        return Ok(None);
    };
    let connections = server
        .db
        .notion_connections(account_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let Some(conn) = connections
        .into_iter()
        .find(|c| c.token_invalid_since.is_none())
    else {
        return Ok(None);
    };

    let token = server
        .db
        .connection_token(account_id, conn.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("connection vanished"))?;

    match crate::notion_api::pages(&token).await {
        Ok(pages) => {
            // Only asked when there is an empty list to explain -- a second
            // Notion round trip on every visit would be a cost paid by
            // everyone for a message almost nobody needs.
            let databases = if pages.is_empty() {
                crate::notion_api::data_source_count(&token)
                    .await
                    .unwrap_or(0)
            } else {
                0
            };
            Ok(Some(Choices {
                connection_id: conn.id,
                workspace: conn.workspace_name,
                pages: pages
                    .into_iter()
                    .map(|p| NotionPage {
                        id: p.id,
                        title: p.title,
                    })
                    .collect(),
                databases,
            }))
        }
        Err(crate::notion_api::Error::Unauthorized) => {
            // Recorded, not just reported. The mark is what stops the bridge
            // retrying a dead grant, and the only place that learns the token
            // is dead is whoever called Notion first.
            server
                .db
                .mark_notion_invalid(conn.id)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            Err(ServerFnError::new(
                "Notion refused this connection. Reconnect to continue.",
            ))
        }
        Err(e) => Err(ServerFnError::new(e.to_string())),
    }
}

/// Links a page: reserves the row, then creates the calendar.
///
/// That order is deliberate and mirrors credential provisioning. The UNIQUE
/// on `(account_id, notion_parent_page)` is the only thing that can decide a
/// race between two tabs, so the database claims first and the calendar is
/// created by whoever won. The reverse order would create two collections
/// and then discover the conflict.
///
/// A failure after the claim releases it. Leaving the row would point a link
/// at a collection that does not exist *and* make every retry fail on the
/// constraint -- one transient error would lock the page out permanently.
#[server]
pub async fn link_page(
    connection_id: i64,
    page_id: String,
    title: String,
) -> Result<String, ServerFnError> {
    let Some((server, account_id)) = crate::pages::me::session().await? else {
        return Err(ServerFnError::new("not signed in"));
    };
    let account = server
        .db
        .account(account_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("session points at no account"))?;
    let (Some(username), Some(password)) = (account.caldav_username, account.caldav_password)
    else {
        return Err(ServerFnError::new(
            "no CalDAV credential yet; open the account page first",
        ));
    };

    // Owned by this account, checked in the query rather than assumed from
    // the form field -- `connection_id` arrives from the browser.
    if server
        .db
        .connection_token(account_id, connection_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .is_none()
    {
        return Err(ServerFnError::new("no such connection"));
    }

    let slug = slug_for(&page_id);
    let collection = format!("/calendars/{username}/{slug}/");

    let Some(link_id) = server
        .db
        .claim_link(account_id, connection_id, &page_id, &title, &collection)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    else {
        return Err(ServerFnError::new("that page is already linked"));
    };

    let client = crate::caldav_client::CaldavClient::new(&server.caldav_public_url);
    let outcome = client
        .make_calendar(&username, &password, &collection, &title)
        .await;

    match decide(outcome) {
        Decision::Keep => Ok(collection),
        Decision::Release(message) => {
            server
                .db
                .release_link(link_id)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            Err(ServerFnError::new(message))
        }
    }
}

/// What to do with a claimed link once MKCALENDAR has answered.
#[cfg(feature = "ssr")]
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// The collection exists and belongs to this link.
    Keep,
    /// It does not, so the claim has to go back.
    Release(String),
}

/// Split out from `link_page` so it can be tested.
///
/// It was inline, and that made the one branch most worth proving --
/// releasing the claim on failure -- unreachable from any test: deleting the
/// release call left every test green. A decision this small being wrong is
/// the difference between "try again" and "this page can never be linked".
#[cfg(feature = "ssr")]
pub fn decide(outcome: Result<(), crate::caldav_client::Error>) -> Decision {
    match outcome {
        Ok(()) => Decision::Keep,
        // Already there from an earlier attempt that failed after
        // MKCALENDAR. The collection is the one this link wants, so adopting
        // it is right -- creating a second would leave an orphan, and
        // releasing would strand the collection with no link naming it.
        Err(crate::caldav_client::Error::AlreadyExists) => Decision::Keep,
        Err(e) => Decision::Release(format!("could not create calendar: {e}")),
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use crate::caldav_client::Error;

    #[test]
    fn a_created_calendar_keeps_its_claim() {
        assert_eq!(decide(Ok(())), Decision::Keep);
    }

    #[test]
    fn an_existing_calendar_keeps_its_claim() {
        // The retry path. Releasing here would strand a collection that no
        // link names, and the next attempt would create a second one.
        assert_eq!(decide(Err(Error::AlreadyExists)), Decision::Keep);
    }

    #[test]
    fn every_other_failure_releases_the_claim() {
        // Without this the row survives pointing at a collection that does
        // not exist, and the UNIQUE constraint refuses every retry -- one
        // transient failure locks the page out permanently.
        for e in [
            Error::Unauthorized,
            Error::Other("connection refused".into()),
        ] {
            assert!(matches!(decide(Err(e)), Decision::Release(_)));
        }
    }
}

/// A collection name derived from the Notion page id.
///
/// Derived rather than taken from the title: a title is whatever the user
/// typed, and it becomes a path segment. Deriving it also makes the whole
/// operation idempotent -- a retry names the same collection, so it is the
/// same request rather than a second calendar.
#[cfg(feature = "ssr")]
fn slug_for(page_id: &str) -> String {
    let hex: String = page_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(12)
        .collect::<String>()
        .to_lowercase();
    format!("notion-{hex}")
}

#[component]
pub fn LinkPicker() -> impl IntoView {
    let choices = Resource::new(|| (), |_| notion_choices());
    let link = Action::new(|(conn, page, title): &(i64, String, String)| {
        let (conn, page, title) = (*conn, page.clone(), title.clone());
        async move { link_page(conn, page, title).await }
    });

    view! {
      <Column class="py-8">
        <Title text="Add a calendar"/>
        <h1 class="text-title font-semibold mb-2">"Add a calendar"</h1>
        <Suspense fallback=|| view! { <p class="text-body text-ink-48">"Loading…"</p> }>
            {move || Suspend::new(async move {
                match choices.await {
                    Err(e) => view! {
                        <p data-testid="error" class="text-body text-ink-48 mb-6">{e.to_string()}</p>
                        <ServerLink
                            href="/notion/connect"
                            label="Reconnect Notion"
                            testid="reconnect-notion"
                            button=true
                            class="w-full h-12 inline-flex items-center justify-center rounded-full
                                   bg-action text-canvas text-body font-semibold no-underline"
                        />
                    }.into_any(),
                    Ok(None) => view! {
                        <p class="text-body text-ink-48 mb-6">
                            "Connect a Notion workspace first."
                        </p>
                        <A href="/me" attr:role="button"
                           attr:class="w-full h-12 inline-flex items-center justify-center rounded-full
                                       bg-canvas border border-hairline text-action text-body no-underline">
                            "Back"
                        </A>
                    }.into_any(),
                    Ok(Some(c)) => view! { <PageList choices=c link=link/> }.into_any(),
                }
            })}
        </Suspense>
      </Column>
    }
}

#[component]
fn PageList(
    choices: Choices,
    link: Action<(i64, String, String), Result<String, ServerFnError>>,
) -> impl IntoView {
    let result = link.value();
    let conn = choices.connection_id;
    let is_empty = choices.pages.is_empty();
    let db_count = choices.databases;
    view! {
        <p class="text-body text-ink-48 mb-6">
            "The calendar database is created under the page you pick, in "
            {choices.workspace.clone()} "."
        </p>

        <Show when=move || result.get().is_some()>
            <div data-testid="link-result" class="rounded-lg bg-canvas border border-hairline p-4 mb-6">
                {move || match result.get() {
                    Some(Ok(uri)) => view! {
                        <div class="text-body mb-1">"Calendar created."</div>
                        // Shown immediately, because this is the thing a
                        // client that cannot discover has to be given.
                        <div data-testid="calendar-url" class="text-caption text-ink-48 break-all">
                            {uri}
                        </div>
                    }.into_any(),
                    Some(Err(e)) => view! {
                        <div data-testid="link-error" class="text-body">{e.to_string()}</div>
                    }.into_any(),
                    None => view! { <div/> }.into_any(),
                }}
            </div>
        </Show>

        <Show when=move || is_empty>
            <EmptyPicker databases=db_count/>
        </Show>

        <div class="rounded-lg bg-canvas border border-hairline divide-y divide-divider-soft">
            <For each=move || choices.pages.clone() key=|p| p.id.clone() let:page>
                {
                    let (id, title) = (page.id.clone(), page.title.clone());
                    view! {
                        <div class="px-4 py-3 flex items-center gap-3">
                            <div data-testid="page-title" class="text-body flex-1 break-all">
                                {page.title.clone()}
                            </div>
                            <PendingButton
                                pending=link.pending()
                                label="Add"
                                busy="Adding"
                                testid="add-page"
                                class="h-11 px-5 inline-flex items-center justify-center gap-2
                                       rounded-full bg-action text-canvas text-body
                                       disabled:opacity-40 disabled:cursor-not-allowed"
                                on_click=Callback::new(move |()| {
                                    link.dispatch((conn, id.clone(), title.clone()));
                                })
                            />
                        </div>
                    }
                }
            </For>
        </div>
    }
}

/// Why the list is empty, which is not the same question as what is in it.
///
/// Notion's consent dialog lists pages and databases together and cannot be
/// filtered, so sharing a database instead of a page is the easiest mistake
/// to make here -- and its symptom is a blank screen. Saying "you shared 3
/// databases and no pages" turns that into an instruction.
#[component]
fn EmptyPicker(databases: usize) -> impl IntoView {
    // Computed before the macro: `databases > 0` inside `view!` has its `>`
    // read as the end of a tag.
    let shared_databases = databases > 0;
    let line = format!(
        "You shared {databases} database{} and no pages.",
        if databases == 1 { "" } else { "s" },
    );

    view! {
        <div class="rounded-lg bg-canvas border border-hairline p-5 mb-6">
            <Show
                when=move || shared_databases
                fallback=|| {
                    view! {
                        <div>
                            <p data-testid="empty-none" class="text-body mb-2">
                                "No pages shared yet."
                            </p>
                            <p class="text-body text-ink-48">
                                "Go back to Notion and share at least one page with this connection."
                            </p>
                        </div>
                    }
                }
            >
                <div>
                    <p data-testid="empty-databases" class="text-body mb-2">{line.clone()}</p>
                    <p class="text-body text-ink-48">
                        "This needs a page to put a calendar under -- it creates its own
                         database inside it. Share a page instead; an empty one is fine."
                    </p>
                </div>
            </Show>
            <div class="mt-4">
                <ServerLink
                    href="/notion/connect"
                    label="Change what is shared"
                    testid="reshare"
                    button=true
                    class="w-full h-12 inline-flex items-center justify-center rounded-full
                           bg-action text-canvas text-body font-semibold no-underline"
                />
            </div>
        </div>
    }
}
