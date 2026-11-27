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
    db.create_session(&sid, id).await.unwrap();
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
    db.create_session(&sid, id).await.unwrap();
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
    db.start_notion_flow(&state, id).await.unwrap();

    assert_eq!(db.finish_notion_flow(&state).await.unwrap(), Some(id));
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
