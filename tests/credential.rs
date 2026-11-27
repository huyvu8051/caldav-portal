//! The invariants the credentials page rests on.
//!
//! Two of these are about a race that is invisible in a single-threaded test
//! and in single-tab use: the database is the thing that decides who
//! provisions, and the queries have to earn that.

use caldav_portal::db::Db;

async fn db() -> Db {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:bench@127.0.0.1:55432/caldav_portal".into());
    let db = Db::connect(&url).await.expect("Postgres must be running");
    db.migrate().await.expect("migrate");
    db
}

/// Unique per test run so tests can share one database without a schema
/// dance. Both of these have to be unique, not just the subject: a fixed
/// `caldav_username` collides with its own previous run on the UNIQUE index,
/// which looks like a failing invariant and is a failing test fixture.
fn sub() -> String {
    format!("test-{}", caldav_portal::oidc::random_token())
}

fn username() -> String {
    format!("tu{}", &caldav_portal::oidc::random_token()[..16])
}

#[tokio::test]
async fn migrating_twice_is_not_an_error() {
    // caldav-rs crash-looped on exactly this: a migration that is fine once
    // and fatal on the second process start.
    let db = db().await;
    db.migrate().await.expect("second migrate");
    db.migrate().await.expect("third migrate");
}

#[tokio::test]
async fn only_one_caller_can_claim_a_credential() {
    let db = db().await;
    let id = db.upsert_account(&sub(), "a@example.com").await.unwrap();

    let (first_name, second_name) = (username(), username());
    let first = db
        .claim_credential(id, &first_name, "pw-first")
        .await
        .unwrap();
    let second = db
        .claim_credential(id, &second_name, "pw-second")
        .await
        .unwrap();

    assert!(first, "the first caller must win the claim");
    assert!(
        !second,
        "a second claim must lose -- otherwise two tabs both call caldav-rs \
         and the loser's password is stored here while the winner's hash is \
         stored there"
    );

    let account = db.account(id).await.unwrap().unwrap();
    assert_eq!(
        account.caldav_username.as_deref(),
        Some(first_name.as_str())
    );
    assert_eq!(account.caldav_password.as_deref(), Some("pw-first"));
}

#[tokio::test]
async fn signing_in_again_updates_the_email_and_keeps_the_account() {
    let db = db().await;
    let s = sub();
    let first = db.upsert_account(&s, "old@example.com").await.unwrap();
    let second = db.upsert_account(&s, "new@example.com").await.unwrap();
    assert_eq!(first, second, "the same subject must be the same account");
    assert_eq!(
        db.account(first).await.unwrap().unwrap().email,
        "new@example.com"
    );
}

#[tokio::test]
async fn a_login_flow_can_only_be_finished_once() {
    // The callback consumes the row before exchanging the code, so a replayed
    // callback finds nothing. Without that, the same authorization code could
    // be presented twice in the window between.
    let db = db().await;
    let state = caldav_portal::oidc::random_token();
    db.start_login(&state, "verifier", "nonce").await.unwrap();

    let first = db.finish_login(&state).await.unwrap();
    let second = db.finish_login(&state).await.unwrap();

    assert_eq!(
        first,
        Some(("verifier".to_string(), "nonce".to_string())),
        "the first callback must get the verifier and nonce back"
    );
    assert!(second.is_none(), "a replayed callback must find nothing");
}

#[tokio::test]
async fn an_expired_session_does_not_resolve_to_an_account() {
    let db = db().await;
    let id = db.upsert_account(&sub(), "a@example.com").await.unwrap();
    let sid = caldav_portal::oidc::random_token();
    db.create_session(&sid, id, "fake.id.token").await.unwrap();
    assert_eq!(db.session_account(&sid).await.unwrap(), Some(id));

    db.expire_session_for_test(&sid).await.unwrap();
    assert_eq!(
        db.session_account(&sid).await.unwrap(),
        None,
        "expiry is checked in the query; a caller must not have to remember to"
    );
}

#[tokio::test]
async fn signing_out_invalidates_the_session_server_side() {
    let db = db().await;
    let id = db.upsert_account(&sub(), "a@example.com").await.unwrap();
    let sid = caldav_portal::oidc::random_token();
    db.create_session(&sid, id, "fake.id.token").await.unwrap();
    db.delete_session(&sid).await.unwrap();
    assert_eq!(
        db.session_account(&sid).await.unwrap(),
        None,
        "clearing only the cookie would leave this id working for whoever has it"
    );
}

#[test]
fn a_generated_password_has_no_look_alike_characters() {
    // Someone reads this off a screen and types it into iOS. Deciding whether
    // a glyph is a 1 or an l is not a thing to make them do.
    for _ in 0..200 {
        let pw = caldav_portal::oidc::readable_password();
        assert_eq!(pw.len(), 20);
        for c in pw.chars() {
            assert!(!"0o1li".contains(c), "{pw} contains the look-alike {c:?}");
            assert!(c.is_ascii_alphanumeric(), "{pw} has {c:?}");
        }
    }
}

#[test]
fn session_ids_do_not_repeat() {
    use std::collections::HashSet;
    let seen: HashSet<String> = (0..1000)
        .map(|_| caldav_portal::oidc::random_token())
        .collect();
    assert_eq!(seen.len(), 1000);
}

#[tokio::test]
async fn reconnecting_clears_the_invalid_mark() {
    // This is the repair path. A user whose Notion token was revoked fixes it
    // by reconnecting, so the upsert has to clear `token_invalid_since` --
    // otherwise a successful reconnect is indistinguishable from the broken
    // state it was meant to fix, and the sync loop keeps skipping the link.
    let db = db().await;
    let id = db.upsert_account(&sub(), "a@example.com").await.unwrap();
    let ws = username();

    let conn = db
        .put_notion_connection(id, "tok-1", &ws, "Workspace", "bot")
        .await
        .unwrap();
    db.mark_notion_invalid(conn).await.unwrap();
    assert!(
        db.notion_connections(id).await.unwrap()[0]
            .token_invalid_since
            .is_some(),
        "the mark must be set before the repair is meaningful"
    );

    let again = db
        .put_notion_connection(id, "tok-2", &ws, "Workspace", "bot")
        .await
        .unwrap();
    assert_eq!(again, conn, "reconnecting the same workspace replaces it");
    let rows = db.notion_connections(id).await.unwrap();
    assert_eq!(rows.len(), 1, "no duplicate row per workspace");
    assert!(
        rows[0].token_invalid_since.is_none(),
        "the mark must be cleared"
    );
}

#[tokio::test]
async fn marking_invalid_records_when_it_first_failed() {
    // Not when it was last retried. A loop that re-marks on every tick would
    // make "your calendar has been stale since" always read as "just now".
    let db = db().await;
    let id = db.upsert_account(&sub(), "a@example.com").await.unwrap();
    let conn = db
        .put_notion_connection(id, "tok", &username(), "W", "b")
        .await
        .unwrap();

    db.mark_notion_invalid(conn).await.unwrap();
    let first = db.notion_connections(id).await.unwrap()[0].token_invalid_since;
    db.mark_notion_invalid(conn).await.unwrap();
    let second = db.notion_connections(id).await.unwrap()[0].token_invalid_since;

    assert_eq!(first, second, "the timestamp must not move on a retry");
}

#[tokio::test]
async fn a_notion_flow_can_only_be_finished_once() {
    let db = db().await;
    let id = db.upsert_account(&sub(), "a@example.com").await.unwrap();
    let state = caldav_portal::oidc::random_token();
    db.start_notion_flow(&state, id, "add").await.unwrap();

    assert_eq!(
        db.finish_notion_flow(&state).await.unwrap(),
        Some((id, "add".to_string()))
    );
    assert_eq!(
        db.finish_notion_flow(&state).await.unwrap(),
        None,
        "a replayed callback must not attach a second grant"
    );
}

#[tokio::test]
async fn one_account_cannot_delete_another_accounts_connection() {
    let db = db().await;
    let mine = db.upsert_account(&sub(), "a@example.com").await.unwrap();
    let theirs = db.upsert_account(&sub(), "b@example.com").await.unwrap();
    let conn = db
        .put_notion_connection(theirs, "tok", &username(), "W", "b")
        .await
        .unwrap();

    // Scoped by account in the WHERE clause, not just by id: an id alone is
    // guessable and would be enough.
    db.delete_notion_connection(mine, conn).await.unwrap();
    assert_eq!(db.notion_connections(theirs).await.unwrap().len(), 1);

    db.delete_notion_connection(theirs, conn).await.unwrap();
    assert_eq!(db.notion_connections(theirs).await.unwrap().len(), 0);
}

#[tokio::test]
async fn a_failed_calendar_creation_does_not_lock_the_page_out() {
    // The claim comes first so a race has one winner, which means a failure
    // after the claim has to release it. Leaving the row would point a link
    // at a collection that does not exist *and* make every retry fail on the
    // UNIQUE constraint -- one transient error would lock that page out for
    // good.
    let db = db().await;
    let id = db.upsert_account(&sub(), "a@example.com").await.unwrap();
    let conn = db
        .put_notion_connection(id, "tok", &username(), "W", "b")
        .await
        .unwrap();
    let page = format!("page-{}", caldav_portal::oidc::random_token());

    // Generated, not fixed: `caldav_calendar_uri` is UNIQUE, so a literal
    // collides with this test's own previous run. The same mistake cost a
    // red suite once already.
    let uri = format!("/calendars/u1/{}/", username());

    let first = db
        .claim_link(id, conn, &page, "Work", &uri)
        .await
        .unwrap()
        .expect("the first claim wins");
    db.release_link(first).await.unwrap();

    let again = db.claim_link(id, conn, &page, "Work", &uri).await.unwrap();
    assert!(again.is_some(), "a retry after a release must succeed");
}

#[tokio::test]
async fn the_same_page_cannot_be_linked_twice() {
    let db = db().await;
    let id = db.upsert_account(&sub(), "a@example.com").await.unwrap();
    let conn = db
        .put_notion_connection(id, "tok", &username(), "W", "b")
        .await
        .unwrap();
    let page = format!("page-{}", caldav_portal::oidc::random_token());

    assert!(db
        .claim_link(
            id,
            conn,
            &page,
            "Work",
            &format!("/calendars/u1/{}/", username())
        )
        .await
        .unwrap()
        .is_some());
    assert!(
        db.claim_link(
            id,
            conn,
            &page,
            "Work",
            &format!("/calendars/u1/{}/", username())
        )
        .await
        .unwrap()
        .is_none(),
        "two calendars racing to own one Notion database is the thing the \
         constraint exists to stop"
    );
    assert_eq!(db.links(id).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_connection_token_is_not_readable_across_accounts() {
    let db = db().await;
    let mine = db.upsert_account(&sub(), "a@example.com").await.unwrap();
    let theirs = db.upsert_account(&sub(), "b@example.com").await.unwrap();
    let conn = db
        .put_notion_connection(theirs, "their-token", &username(), "W", "b")
        .await
        .unwrap();

    assert_eq!(
        db.connection_token(mine, conn).await.unwrap(),
        None,
        "reading by connection id alone would hand over someone else's \
         Notion token"
    );
    assert_eq!(
        db.connection_token(theirs, conn).await.unwrap().as_deref(),
        Some("their-token")
    );
}

/// Links to server routes must opt out of the client router.
///
/// A source-level check, which is unusual and earned. leptos_router
/// intercepts every same-origin anchor click and resolves it against the
/// *client* route table. `/signin`, `/signout` and `/notion/connect` are
/// plain axum handlers returning 303, so they are not in that table: the
/// router finds no match, renders NotFound, and the request never leaves the
/// browser. The URL bar changes, so it reads as a routing bug rather than a
/// link that was never followed.
///
/// Every curl of those paths answered correctly, every other test passed, and
/// the page said "Not found". Nothing below the browser can see this, which
/// is why the check is on the source.
#[test]
fn server_routes_are_only_linked_through_server_link() {
    const SERVER_ROUTES: [&str; 4] = ["/signin", "/signout", "/notion/connect", "/notion/callback"];
    let mut offenders = Vec::new();

    for entry in std::fs::read_dir("src/pages").expect("src/pages") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read");
        // Split on `<a` followed by any whitespace -- not `"<a "`, which was
        // the first spelling and missed every anchor written across lines.
        // A mutation check caught that: putting a raw multi-line anchor back
        // left this test green.
        let chunks: Vec<&str> = src
            .match_indices("<a")
            .filter(|(i, _)| {
                src[i + 2..]
                    .chars()
                    .next()
                    .map(|c| c.is_whitespace())
                    .unwrap_or(false)
            })
            .map(|(i, _)| &src[i..])
            .collect();
        for chunk in chunks {
            let head: String = chunk.chars().take(400).collect();
            for route in SERVER_ROUTES {
                if head.contains(&format!("\"{route}\"")) && !head.contains("rel=\"external\"") {
                    offenders.push(format!(
                        "{}: <a> to {route} without rel=external",
                        path.display()
                    ));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "use ui::ServerLink for these -- {offenders:#?}"
    );
}

#[tokio::test]
async fn the_id_token_is_kept_for_logout_and_dies_with_the_session() {
    // Signing out has to end the session at Keycloak too, and that needs
    // `id_token_hint`. Without it the provider keeps its own SSO session, the
    // next sign-in is recognised silently, and "log out" logs you straight
    // back in as the same account.
    let db = db().await;
    let id = db.upsert_account(&sub(), "a@example.com").await.unwrap();
    let sid = caldav_portal::oidc::random_token();

    db.create_session(&sid, id, "header.payload.signature")
        .await
        .unwrap();
    assert_eq!(
        db.session_id_token(&sid).await.unwrap().as_deref(),
        Some("header.payload.signature")
    );

    db.delete_session(&sid).await.unwrap();
    assert_eq!(
        db.session_id_token(&sid).await.unwrap(),
        None,
        "it is a credential; it must not outlive the session row"
    );
}

#[tokio::test]
async fn a_reconnect_flow_is_remembered_as_one() {
    // "Add a calendar" and "Reconnect" are the same OAuth round trip, and
    // Notion duplicates the template on both -- so without the intent coming
    // back, repairing a dead connection would silently add a second
    // calendar. Fixing something would grow the list.
    let db = db().await;
    let id = db.upsert_account(&sub(), "a@example.com").await.unwrap();

    let add = caldav_portal::oidc::random_token();
    let again = caldav_portal::oidc::random_token();
    db.start_notion_flow(&add, id, "add").await.unwrap();
    db.start_notion_flow(&again, id, "reconnect").await.unwrap();

    assert_eq!(
        db.finish_notion_flow(&add).await.unwrap(),
        Some((id, "add".to_string()))
    );
    assert_eq!(
        db.finish_notion_flow(&again).await.unwrap(),
        Some((id, "reconnect".to_string())),
        "the callback has to know which button was pressed"
    );
}
