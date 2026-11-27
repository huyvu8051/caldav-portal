//! Keycloak login: the authorization-code flow with PKCE.
//!
//! Three things travel across the redirect and each guards something
//! different, which is worth writing down because they are easy to conflate:
//!
//! - **state** proves the callback belongs to a login this server started.
//!   Without it anyone can send a victim's browser to `/callback` with their
//!   own code and log the victim into the attacker's account.
//! - **PKCE verifier** proves the party redeeming the code is the party that
//!   asked for it. It matters even for a confidential client, because the
//!   code arrives in a URL and URLs end up in logs and history.
//! - **nonce** binds the returned ID token to this request, so a token minted
//!   for some other session cannot be replayed into this one.
//!
//! All three live in `login_flow`, and reading them deletes the row.

use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenResponse,
};

pub type Client = CoreClient<
    openidconnect::EndpointSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointMaybeSet,
    openidconnect::EndpointMaybeSet,
>;

#[derive(Clone)]
pub struct Oidc {
    pub client: Client,
    pub http: openidconnect::reqwest::Client,
}

#[derive(Debug)]
pub struct Identity {
    pub subject: String,
    pub email: String,
}

/// Discovery with a bounded retry.
///
/// Keycloak and the portal come up together, and on a cold cluster the portal
/// usually wins. Failing to start on that race would be a crash loop that
/// resolves itself only because the kubelet keeps trying -- which is the same
/// retry, with the restart counter as its only log.
pub async fn connect(
    issuer: &str,
    client_id: &str,
    client_secret: &str,
    redirect_url: &str,
) -> anyhow::Result<Oidc> {
    // Redirects disabled: following them from a discovery URL is the standard
    // SSRF foot-gun, and the crate's own example says so.
    let http = openidconnect::reqwest::ClientBuilder::new()
        .redirect(openidconnect::reqwest::redirect::Policy::none())
        .build()?;

    let issuer_url = IssuerUrl::new(issuer.to_string())?;
    let mut backoff = std::time::Duration::from_secs(2);
    let mut last: Option<String> = None;
    for attempt in 1..=8u32 {
        match CoreProviderMetadata::discover_async(issuer_url.clone(), &http).await {
            Ok(meta) => {
                let client = CoreClient::from_provider_metadata(
                    meta,
                    ClientId::new(client_id.to_string()),
                    Some(ClientSecret::new(client_secret.to_string())),
                )
                .set_redirect_uri(RedirectUrl::new(redirect_url.to_string())?);
                return Ok(Oidc { client, http });
            }
            Err(e) => {
                last = Some(e.to_string());
                tracing::warn!(attempt, error = %e, ?backoff, "OIDC discovery failed; retrying");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(std::time::Duration::from_secs(30));
            }
        }
    }
    anyhow::bail!(
        "OIDC discovery never succeeded for {issuer}: {}",
        last.unwrap_or_default()
    )
}

/// Returns the URL to send the browser to, plus the three values the callback
/// will need back.
pub fn start(oidc: &Oidc) -> (String, String, String, String) {
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, csrf, nonce) = oidc
        .client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        // `openid` is not added here: openidconnect puts it in every
        // authorize URL itself, and adding it again sent Keycloak
        // `scope=openid openid email profile`.
        .add_scope(Scope::new("email".into()))
        .add_scope(Scope::new("profile".into()))
        .set_pkce_challenge(challenge)
        .url();
    (
        url.to_string(),
        csrf.secret().clone(),
        verifier.secret().clone(),
        nonce.secret().clone(),
    )
}

/// Exchange the code and verify the ID token. Anything that fails here is an
/// authentication failure, never a partial login.
pub async fn finish(
    oidc: &Oidc,
    code: String,
    verifier: String,
    nonce: String,
) -> anyhow::Result<Identity> {
    let tokens = oidc
        .client
        .exchange_code(openidconnect::AuthorizationCode::new(code))?
        .set_pkce_verifier(PkceCodeVerifier::new(verifier))
        .request_async(&oidc.http)
        .await?;

    let id_token = tokens
        .id_token()
        .ok_or_else(|| anyhow::anyhow!("no ID token in the token response"))?;
    // Verifying is what makes this an authentication rather than a fetch: it
    // checks the signature against the discovered JWKS, the issuer, the
    // audience, and the nonce this flow generated.
    let claims = id_token.claims(&oidc.client.id_token_verifier(), &Nonce::new(nonce))?;

    let email = claims
        .email()
        .map(|e| e.as_str().to_string())
        .unwrap_or_default();
    Ok(Identity {
        subject: claims.subject().as_str().to_string(),
        email,
    })
}

/// Unguessable identifiers for sessions. 256 bits from the OS, hex-encoded,
/// which is what lets the session cookie carry nothing but this.
pub fn random_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A CalDAV password a person has to retype into a phone.
///
/// No look-alike characters: someone reading this off a screen and typing it
/// into iOS should not have to decide whether a glyph is a 1, an l or an i.
/// The alphabet drops `i`, `l`, `o`, `0` and `1`, leaving 31 symbols, so 20
/// of them is ~99 bits -- far beyond what matters with argon2 in front.
pub fn readable_password() -> String {
    use rand::RngCore;
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    debug_assert_eq!(ALPHABET.len(), 31);
    // 248 is 31 * 8, so every byte below it maps to a symbol uniformly and
    // the rest are discarded. Taking `% 31` of every byte instead would make
    // the first nine symbols slightly likelier -- harmless at this length,
    // but a biased password generator is not a thing to leave lying around
    // when unbiasing it costs one comparison.
    const LIMIT: u8 = 248;
    let mut out = String::with_capacity(20);
    let mut buf = [0u8; 32];
    while out.len() < 20 {
        rand::rng().fill_bytes(&mut buf);
        for b in buf {
            if out.len() == 20 {
                break;
            }
            if b < LIMIT {
                out.push(ALPHABET[(b % 31) as usize] as char);
            }
        }
    }
    out
}
