//! The parts of sign-in that are redirects rather than pages.
//!
//! These are plain axum handlers, not Leptos routes, because every one of
//! them answers with a 302 and a `Set-Cookie`. Rendering is not involved.

use axum::extract::{Extension, Query};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;

use crate::state::ServerState;

/// Name kept boring on purpose; it carries a random id and nothing else.
pub const SESSION_COOKIE: &str = "portal_session";

fn session_cookie(value: String, secure: bool) -> Cookie<'static> {
    let mut c = Cookie::new(SESSION_COOKIE, value);
    // HttpOnly: no page script has any reason to read this, and not being
    // readable is what stops one XSS from becoming a stolen session.
    c.set_http_only(true);
    // Lax rather than Strict: the login returns from Keycloak as a top-level
    // navigation from another origin, and Strict would withhold the cookie on
    // exactly that request -- the user would land signed in and look signed
    // out.
    c.set_same_site(SameSite::Lax);
    c.set_path("/");
    // Off over plain HTTP, or the browser drops it and development silently
    // never logs in. Driven by the configured base URL rather than guessed.
    c.set_secure(secure);
    c
}

pub async fn signin(Extension(state): Extension<ServerState>) -> Response {
    let (url, csrf, verifier, nonce) = crate::oidc::start(&state.oidc);
    if let Err(e) = state.db.start_login(&csrf, &verifier, &nonce).await {
        tracing::error!(error = %e, "could not record the login flow");
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "sign-in unavailable",
        )
            .into_response();
    }
    Redirect::to(&url).into_response()
}

#[derive(Deserialize)]
pub struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub async fn callback(
    Extension(server): Extension<ServerState>,
    jar: CookieJar,
    Query(q): Query<Callback>,
) -> Response {
    if let Some(e) = q.error {
        tracing::warn!(error = %e, "Keycloak refused the login");
        return Redirect::to("/?error=denied").into_response();
    }
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return Redirect::to("/?error=incomplete").into_response();
    };

    // Consumes the row. A replayed callback finds nothing and is refused,
    // which is the point of doing this before the token exchange.
    let Ok(Some((verifier, nonce))) = server.db.finish_login(&state).await else {
        tracing::warn!("callback with an unknown or expired state");
        return Redirect::to("/?error=expired").into_response();
    };

    let identity = match crate::oidc::finish(&server.oidc, code, verifier, nonce).await {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!(error = %e, "token exchange or ID token verification failed");
            return Redirect::to("/?error=verify").into_response();
        }
    };

    let account_id = match server
        .db
        .upsert_account(&identity.subject, &identity.email)
        .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(error = %e, "could not record the account");
            return Redirect::to("/?error=db").into_response();
        }
    };

    let sid = crate::oidc::random_token();
    if let Err(e) = server.db.create_session(&sid, account_id).await {
        tracing::error!(error = %e, "could not create the session");
        return Redirect::to("/?error=db").into_response();
    }

    (
        jar.add(session_cookie(sid, server.secure_cookies)),
        Redirect::to("/me"),
    )
        .into_response()
}

pub async fn signout(Extension(server): Extension<ServerState>, jar: CookieJar) -> Response {
    if let Some(c) = jar.get(SESSION_COOKIE) {
        // Deleted server-side as well as in the browser. Clearing only the
        // cookie leaves a session id that still works for anyone holding it.
        let _ = server.db.delete_session(c.value()).await;
    }
    (jar.remove(Cookie::from(SESSION_COOKIE)), Redirect::to("/")).into_response()
}

/// Resolves the session cookie to an account. Shared by the Notion routes,
/// which unlike sign-in start from someone already signed in.
async fn account_of(server: &ServerState, jar: &CookieJar) -> Option<i64> {
    let cookie = jar.get(SESSION_COOKIE)?;
    server
        .db
        .session_account(cookie.value())
        .await
        .ok()
        .flatten()
}

pub async fn notion_connect(Extension(server): Extension<ServerState>, jar: CookieJar) -> Response {
    let Some(account_id) = account_of(&server, &jar).await else {
        // Not an error page: someone whose session expired while the tab was
        // open should land on sign-in, not on an explanation.
        return Redirect::to("/signin").into_response();
    };
    let state = crate::oidc::random_token();
    if let Err(e) = server.db.start_notion_flow(&state, account_id).await {
        tracing::error!(error = %e, "could not record the notion flow");
        return Redirect::to("/me?error=db").into_response();
    }
    Redirect::to(&server.notion.authorize_url(&state)).into_response()
}

#[derive(Deserialize)]
pub struct NotionCallback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub async fn notion_callback(
    Extension(server): Extension<ServerState>,
    Query(q): Query<NotionCallback>,
) -> Response {
    if let Some(e) = q.error {
        // `access_denied` is someone clicking Cancel, which is a choice
        // rather than a fault.
        tracing::info!(error = %e, "notion connect was not completed");
        return Redirect::to("/me?notion=cancelled").into_response();
    }
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return Redirect::to("/me?notion=incomplete").into_response();
    };

    // Consumed before the exchange, so a replayed callback cannot attach a
    // second grant. This is also what ties the grant to the account that
    // started the flow rather than to whoever happens to be holding a cookie.
    let Ok(Some(account_id)) = server.db.finish_notion_flow(&state).await else {
        tracing::warn!("notion callback with an unknown or expired state");
        return Redirect::to("/me?notion=expired").into_response();
    };

    let grant = match server.notion.exchange(&code).await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!(error = %e, "notion token exchange failed");
            return Redirect::to("/me?notion=exchange").into_response();
        }
    };

    if let Err(e) = server
        .db
        .put_notion_connection(
            account_id,
            &grant.access_token,
            &grant.workspace_id,
            grant.workspace_name.as_deref().unwrap_or_default(),
            grant.bot_id.as_deref().unwrap_or_default(),
        )
        .await
    {
        tracing::error!(error = %e, "could not record the notion connection");
        return Redirect::to("/me?notion=db").into_response();
    }
    Redirect::to("/me?notion=connected").into_response()
}
