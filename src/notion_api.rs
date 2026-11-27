//! The two Notion calls the portal makes on a user's behalf.
//!
//! Everything else Notion-shaped lives in `notion-bridge`: creating the
//! calendar database, reading and writing pages. The portal only has to show
//! someone the pages they could put a calendar under, and notice when their
//! grant has been revoked.

use serde::Deserialize;

const API: &str = "https://api.notion.com/v1";
/// Matches `notion-bridge`'s. Databases became data sources in this version,
/// and a mismatch between the two services would mean the portal listing
/// pages the bridge cannot write to.
const VERSION: &str = "2025-09-03";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub id: String,
    pub title: String,
}

#[derive(Debug)]
pub enum Error {
    /// Notion refused the token. Distinguished from everything else because
    /// it is the one failure with a user-facing repair: reconnect.
    Unauthorized,
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unauthorized => write!(f, "Notion refused this connection"),
            Error::Other(s) => write!(f, "{s}"),
        }
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("reqwest client builds")
}

/// What Notion duplicated, so the link can be named after it.
///
/// `duplicated_template_id` is a **database** id -- established by asking,
/// not by reading documentation: `/v1/databases/{id}` answers and
/// `/v1/pages/{id}` returns `validation_error`. Which matters, because a page
/// id would have meant searching inside it for the database.
pub async fn duplicated_database(token: &str, database_id: &str) -> Result<Database, Error> {
    let res = client()
        .get(format!("{API}/databases/{database_id}"))
        .bearer_auth(token)
        .header("Notion-Version", VERSION)
        .send()
        .await
        .map_err(|e| Error::Other(e.to_string()))?;

    if res.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Error::Unauthorized);
    }
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(Error::Other(format!("database: HTTP {status}: {body}")));
    }

    let body: DatabaseResponse = res
        .json()
        .await
        .map_err(|e| Error::Other(format!("database response: {e}")))?;

    let title: String = body
        .title
        .iter()
        .filter_map(|t| t.plain_text.as_deref())
        .collect();
    Ok(Database {
        id: database_id.to_string(),
        title: if title.trim().is_empty() {
            "Notion calendar".to_string()
        } else {
            title
        },
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Database {
    pub id: String,
    pub title: String,
}

#[derive(Deserialize)]
struct DatabaseResponse {
    #[serde(default)]
    title: Vec<RichText>,
}

#[derive(Deserialize)]
struct RichText {
    #[serde(default)]
    plain_text: Option<String>,
}
