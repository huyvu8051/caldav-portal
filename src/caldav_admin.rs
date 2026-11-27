//! Client for caldav-rs's provisioning listener.
//!
//! Two calls, matching the two routes that exist there. The endpoint is on a
//! port the Ingress does not route, so this URL is always an in-cluster one
//! (or, in development, loopback).

#[derive(Clone)]
pub struct CaldavAdmin {
    base: String,
    token: String,
    http: reqwest::Client,
}

#[derive(Debug)]
pub enum Error {
    /// The username is taken. Distinguished from every other failure because
    /// it is the one the caller can do something about.
    Conflict,
    NotFound,
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Conflict => write!(f, "username already taken"),
            Error::NotFound => write!(f, "no such principal"),
            Error::Other(s) => write!(f, "{s}"),
        }
    }
}

impl CaldavAdmin {
    pub fn new(base: &str, token: &str) -> Self {
        CaldavAdmin {
            base: base.trim_end_matches('/').to_string(),
            token: token.to_string(),
            // A provisioning call that hangs would hang the sign-in page with
            // it; the portal would rather show an error than a spinner that
            // never resolves.
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("reqwest client builds"),
        }
    }

    pub async fn create_principal(
        &self,
        username: &str,
        password: &str,
        display_name: &str,
    ) -> Result<(), Error> {
        let res = self
            .http
            .post(format!("{}/admin/principals", self.base))
            .bearer_auth(&self.token)
            .json(&serde_json::json!({
                "username": username,
                "password": password,
                "display_name": display_name,
            }))
            .send()
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
        match res.status().as_u16() {
            201 => Ok(()),
            409 => Err(Error::Conflict),
            s => Err(Error::Other(format!("create_principal: HTTP {s}"))),
        }
    }

    pub async fn set_password(&self, username: &str, password: &str) -> Result<(), Error> {
        let res = self
            .http
            .post(format!(
                "{}/admin/principals/{username}/password",
                self.base
            ))
            .bearer_auth(&self.token)
            .json(&serde_json::json!({ "password": password }))
            .send()
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
        match res.status().as_u16() {
            204 => Ok(()),
            404 => Err(Error::NotFound),
            s => Err(Error::Other(format!("set_password: HTTP {s}"))),
        }
    }
}
