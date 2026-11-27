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
