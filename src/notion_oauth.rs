//! Notion's OAuth, which is not the OIDC flow next door.
//!
//! Three differences that matter:
//!
//! - **No PKCE.** Notion authenticates the token exchange with HTTP Basic
//!   using the client id and secret. `state` is still required, and still for
//!   the same reason: without it anyone can hand a victim's browser a code
//!   and attach their own workspace to the victim's account.
//! - **No ID token.** The token response carries the workspace itself --
//!   `workspace_id`, `workspace_name`, `bot_id` -- so there is nothing to
//!   verify a signature on and nothing that needs one.
//! - **`owner=user`** is required on the authorize URL. Without it Notion
//!   issues an internal-integration grant, which is tied to one workspace
//!   rather than to the person who approved it.

use serde::Deserialize;

const AUTHORIZE: &str = "https://api.notion.com/v1/oauth/authorize";
const TOKEN: &str = "https://api.notion.com/v1/oauth/token";

#[derive(Clone)]
pub struct NotionOauth {
    pub client_id: String,
    client_secret: String,
    pub redirect_uri: String,
    http: reqwest::Client,
}

#[derive(Debug, Deserialize)]
pub struct Grant {
    pub access_token: String,
    #[serde(default)]
    pub workspace_id: String,
    #[serde(default)]
    pub workspace_name: Option<String>,
    #[serde(default)]
    pub bot_id: Option<String>,
    /// What Notion duplicated into the workspace during consent, when the
    /// integration is configured with a template.
    ///
    /// This is the whole of flow B: the calendar database arrives with the
    /// grant instead of being created later, so there is nothing to pick and
    /// no window where the portal has reported success but the database does
    /// not exist yet.
    ///
    /// Whether it names a page or the database itself is not documented
    /// clearly enough to build on, so it is captured and logged before being
    /// relied upon.
    #[serde(default)]
    pub duplicated_template_id: Option<String>,
}

impl NotionOauth {
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        NotionOauth {
            client_id,
            client_secret,
            redirect_uri,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("reqwest client builds"),
        }
    }

    pub fn authorize_url(&self, state: &str) -> String {
        let q = [
            ("client_id", self.client_id.as_str()),
            ("response_type", "code"),
            ("owner", "user"),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("state", state),
        ];
        let query: Vec<String> = q
            .iter()
            .map(|(k, v)| format!("{k}={}", urlencode(v)))
            .collect();
        format!("{AUTHORIZE}?{}", query.join("&"))
    }

    /// Exchanges the code. Basic auth with the client credentials, which is
    /// what Notion documents and what replaces PKCE here.
    pub async fn exchange(&self, code: &str) -> Result<Grant, String> {
        let res = self
            .http
            .post(TOKEN)
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .json(&serde_json::json!({
                "grant_type": "authorization_code",
                "code": code,
                "redirect_uri": self.redirect_uri,
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = res.status();
        let body = res.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            // The body carries Notion's own error code, which is the only
            // thing that distinguishes "the user cancelled" from "the
            // redirect URI does not match what is registered" -- and the
            // second is a configuration error that would otherwise look like
            // the first.
            return Err(format!("token exchange: HTTP {status}: {body}"));
        }
        serde_json::from_str::<Grant>(&body).map_err(|e| format!("token response: {e}"))
    }
}

/// Percent-encoding for query values. Small enough not to be worth a
/// dependency, and the alphabet is the unreserved set from RFC 3986.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_authorize_url_carries_owner_user() {
        let o = NotionOauth::new("cid".into(), "sec".into(), "http://x/cb".into());
        let url = o.authorize_url("st");
        // Without `owner=user` Notion issues an internal-integration grant,
        // which is tied to one workspace instead of to the person approving
        // it -- and nothing about the response says so.
        assert!(url.contains("owner=user"), "{url}");
        assert!(url.contains("response_type=code"), "{url}");
        assert!(url.contains("state=st"), "{url}");
    }

    #[test]
    fn the_redirect_uri_is_encoded() {
        let o = NotionOauth::new("cid".into(), "sec".into(), "http://a:1/b?c=d".into());
        let url = o.authorize_url("st");
        assert!(
            url.contains("redirect_uri=http%3A%2F%2Fa%3A1%2Fb%3Fc%3Dd"),
            "{url}"
        );
        // A raw `?` in the value would start a second query string and Notion
        // would read everything after it as its own parameters.
        assert_eq!(url.matches('?').count(), 1, "{url}");
    }

    #[test]
    fn the_client_secret_never_reaches_the_url() {
        let o = NotionOauth::new("cid".into(), "sup3rs3cret".into(), "http://x/cb".into());
        assert!(!o.authorize_url("st").contains("sup3rs3cret"));
    }
}
