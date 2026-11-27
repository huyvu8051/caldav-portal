#![recursion_limit = "1024"]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use axum::Router;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use std::env;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    caldav_portal::init_executor();

    let port = env::var("PORT").unwrap_or_else(|_| "8091".to_string());
    for (key, default) in [
        ("LEPTOS_OUTPUT_NAME", "caldav-portal"),
        ("LEPTOS_SITE_ROOT", "."),
        ("LEPTOS_SITE_PKG_DIR", "pkg"),
        ("LEPTOS_SITE_ADDR", &format!("127.0.0.1:{port}")),
        ("LEPTOS_RELOAD_PORT", "3013"),
    ] {
        if env::var(key).is_err() {
            env::set_var(key, default);
        }
    }
    let leptos_options = leptos::config::get_configuration(None)
        .expect("failed to load leptos config")
        .leptos_options;

    // Everything the handlers and server functions need. Built before the
    // router so a misconfiguration is a refusal to start rather than a 500 on
    // the first person who tries to sign in.
    let database_url =
        env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("DATABASE_URL is not set"))?;
    let db = caldav_portal::db::Db::connect(&database_url).await?;
    db.migrate().await?;

    let base_url = env::var("PORTAL_BASE_URL").map_err(|_| {
        anyhow::anyhow!("PORTAL_BASE_URL is not set (the externally reachable origin)")
    })?;
    let oidc = caldav_portal::oidc::connect(
        &env::var("KEYCLOAK_ISSUER_URL")
            .map_err(|_| anyhow::anyhow!("KEYCLOAK_ISSUER_URL is not set"))?,
        &env::var("KEYCLOAK_CLIENT_ID").unwrap_or_else(|_| "caldav-portal-app".into()),
        &env::var("KEYCLOAK_CLIENT_SECRET")
            .map_err(|_| anyhow::anyhow!("KEYCLOAK_CLIENT_SECRET is not set"))?,
        &format!("{}/callback", base_url.trim_end_matches('/')),
    )
    .await?;
    let notion = caldav_portal::notion_oauth::NotionOauth::new(
        env::var("NOTION_CLIENT_ID").map_err(|_| anyhow::anyhow!("NOTION_CLIENT_ID is not set"))?,
        env::var("NOTION_CLIENT_SECRET")
            .map_err(|_| anyhow::anyhow!("NOTION_CLIENT_SECRET is not set"))?,
        // Not derived from PORTAL_BASE_URL: Notion matches the redirect
        // against its registered list byte for byte, and the registered one
        // is `http://localhost:8091/...` while the base URL a phone uses is
        // not. One is what Notion must see; the other is where people browse.
        env::var("NOTION_REDIRECT_URI")
            .map_err(|_| anyhow::anyhow!("NOTION_REDIRECT_URI is not set"))?,
    );
    let caldav = caldav_portal::caldav_admin::CaldavAdmin::new(
        &env::var("CALDAV_ADMIN_URL")
            .map_err(|_| anyhow::anyhow!("CALDAV_ADMIN_URL is not set"))?,
        &env::var("CALDAV_ADMIN_TOKEN")
            .map_err(|_| anyhow::anyhow!("CALDAV_ADMIN_TOKEN is not set"))?,
    );
    let server_state = caldav_portal::state::ServerState {
        db,
        oidc,
        caldav,
        notion,
        caldav_public_url: env::var("CALDAV_PUBLIC_URL")
            .map_err(|_| anyhow::anyhow!("CALDAV_PUBLIC_URL is not set (what a phone types in)"))?,
        // Driven by the origin rather than guessed: marking the cookie Secure
        // over plain http has the browser drop it, and the login then fails
        // by silently never sticking.
        secure_cookies: base_url.starts_with("https://"),
    };

    let routes = generate_route_list(caldav_portal::App);

    let app = Router::new()
        // Split for the reason caldav-rs splits them: a liveness probe that
        // fails on a dependency hiccup has the kubelet restart a healthy pod
        // and turns a brief outage into a restart loop that outlasts it.
        .route("/healthz", axum::routing::get(|| async { "ok" }))
        .route("/readyz", axum::routing::get(|| async { "ok" }))
        .route("/signin", axum::routing::get(caldav_portal::routes::signin))
        .route(
            "/callback",
            axum::routing::get(caldav_portal::routes::callback),
        )
        .route(
            "/signout",
            axum::routing::get(caldav_portal::routes::signout),
        )
        .route(
            "/notion/connect",
            axum::routing::get(caldav_portal::routes::notion_connect),
        )
        .route(
            "/notion/callback",
            axum::routing::get(caldav_portal::routes::notion_callback),
        )
        .leptos_routes(&leptos_options, routes, {
            let options = leptos_options.clone();
            move || caldav_portal::shell(options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(caldav_portal::shell))
        // An Extension rather than axum state: `with_state` is already spoken
        // for by LeptosOptions, and server functions reach this through
        // `leptos_axum::extract`, which sees extensions.
        .layer(axum::Extension(server_state))
        .with_state(leptos_options.clone());

    // 0.0.0.0 rather than the LEPTOS_SITE_ADDR loopback: in a container the
    // kubelet's probes arrive from off-host, and on a laptop this is what
    // makes the phone on the same wifi able to open it.
    let addr: std::net::SocketAddr = format!("0.0.0.0:{port}").parse()?;
    tracing::info!("caldav-portal listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // No client-side main; see lib.rs's hydrate() instead.
}
