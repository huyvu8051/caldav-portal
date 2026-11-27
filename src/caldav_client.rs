//! The portal as a CalDAV client.
//!
//! Exactly one method, and that is the point. Creating a calendar is
//! `MKCALENDAR`, which caldav-rs already implements and authenticates as the
//! user -- so the portal does it the way any calendar app would, over the
//! protocol, with the account's own credentials.
//!
//! This is why the provisioning endpoint on caldav-rs has two routes and not
//! three. CalDAV has no way to create an account, so that one had to be
//! invented; it has had a way to create a calendar since RFC 4791.

#[derive(Clone)]
pub struct CaldavClient {
    base: String,
    http: reqwest::Client,
}

#[derive(Debug)]
pub enum Error {
    /// The collection already exists. RFC 4918 5.1 makes that a 405 rather
    /// than a silent overwrite of someone's calendar, and the caller wants to
    /// tell it apart from a real failure.
    AlreadyExists,
    Unauthorized,
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::AlreadyExists => write!(f, "that calendar already exists"),
            Error::Unauthorized => write!(f, "the CalDAV credentials were refused"),
            Error::Other(s) => write!(f, "{s}"),
        }
    }
}

impl CaldavClient {
    pub fn new(base: &str) -> Self {
        CaldavClient {
            base: base.trim_end_matches('/').to_string(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("reqwest client builds"),
        }
    }

    /// `MKCALENDAR` at `collection`, as `username`.
    ///
    /// The body sets the display name, which is what a phone shows in its
    /// calendar list. Without it the calendar appears under its path segment,
    /// and `notion-1` is not a name anyone chose.
    pub async fn make_calendar(
        &self,
        username: &str,
        password: &str,
        collection: &str,
        display_name: &str,
    ) -> Result<(), Error> {
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8" ?>
<C:mkcalendar xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:set><D:prop><D:displayname>{}</D:displayname></D:prop></D:set>
</C:mkcalendar>"#,
            escape(display_name)
        );

        let res = self
            .http
            .request(
                reqwest::Method::from_bytes(b"MKCALENDAR").expect("MKCALENDAR is a valid method"),
                format!("{}{}", self.base, collection),
            )
            .basic_auth(username, Some(password))
            .header("content-type", "application/xml; charset=utf-8")
            .body(body)
            .send()
            .await
            .map_err(|e| Error::Other(e.to_string()))?;

        match res.status().as_u16() {
            201 => Ok(()),
            401 => Err(Error::Unauthorized),
            405 => Err(Error::AlreadyExists),
            s => {
                let body = res.text().await.unwrap_or_default();
                Err(Error::Other(format!("MKCALENDAR: HTTP {s}: {body}")))
            }
        }
    }
}

/// XML-escaping for the display name. A user's page is called whatever they
/// called it, and `Q&A` would otherwise produce a body the server rejects as
/// malformed -- which would read as "MKCALENDAR is broken".
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_display_name_with_markup_characters_is_escaped() {
        assert_eq!(escape("Q&A <work>"), "Q&amp;A &lt;work&gt;");
    }
}
