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

/// Pages this grant can see, newest first, for the "where should the calendar
/// live" picker.
///
/// Only pages, never databases: the bridge creates its own database from a
/// fixed template so the columns are addressed by id and survive renaming.
/// Letting someone point at an existing database would mean guessing which
/// of its columns is the date.
pub async fn pages(token: &str) -> Result<Vec<Page>, Error> {
    let res = client()
        .post(format!("{API}/search"))
        .bearer_auth(token)
        .header("Notion-Version", VERSION)
        .json(&serde_json::json!({
            "filter": { "value": "page", "property": "object" },
            "sort": { "direction": "descending", "timestamp": "last_edited_time" },
            "page_size": 50,
        }))
        .send()
        .await
        .map_err(|e| Error::Other(e.to_string()))?;

    if res.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Error::Unauthorized);
    }
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(Error::Other(format!("search: HTTP {status}: {body}")));
    }

    let body: SearchResponse = res
        .json()
        .await
        .map_err(|e| Error::Other(format!("search response: {e}")))?;

    Ok(body
        .results
        .into_iter()
        .filter(|r| !r.archived && !r.in_trash)
        .map(|r| {
            let title = title_of(&r).unwrap_or_else(|| "Untitled".to_string());
            Page { id: r.id, title }
        })
        .collect())
}

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
    id: String,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    in_trash: bool,
    #[serde(default)]
    properties: serde_json::Value,
}

/// A page's title lives under whichever property has `type: "title"`, and
/// that property's name is whatever the user called it. Finding it by type
/// rather than by the name "Name" is the difference between working for
/// everyone and working for people whose workspace is in English.
fn title_of(r: &SearchResult) -> Option<String> {
    let props = r.properties.as_object()?;
    for (_, value) in props {
        if value.get("type").and_then(|t| t.as_str()) != Some("title") {
            continue;
        }
        let parts = value.get("title")?.as_array()?;
        let text: String = parts
            .iter()
            .filter_map(|p| p.get("plain_text").and_then(|t| t.as_str()))
            .collect();
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(props: serde_json::Value) -> SearchResult {
        SearchResult {
            id: "p1".into(),
            archived: false,
            in_trash: false,
            properties: props,
        }
    }

    #[test]
    fn the_title_is_found_by_type_not_by_name() {
        // A workspace in Vietnamese calls it "Tên", not "Name". Matching on
        // the name would show every page in such a workspace as Untitled.
        let r = result(serde_json::json!({
            "Tên": { "type": "title", "title": [{ "plain_text": "Lịch của tôi" }] },
            "Ghi chú": { "type": "rich_text", "rich_text": [] }
        }));
        assert_eq!(title_of(&r).as_deref(), Some("Lịch của tôi"));
    }

    #[test]
    fn a_title_split_across_rich_text_runs_is_joined() {
        // Notion splits a title at every formatting change, so a page called
        // "Work calendar" with one bold word arrives as three runs.
        let r = result(serde_json::json!({
            "Name": { "type": "title", "title": [
                { "plain_text": "Work " },
                { "plain_text": "calendar" }
            ]}
        }));
        assert_eq!(title_of(&r).as_deref(), Some("Work calendar"));
    }

    #[test]
    fn an_empty_title_is_not_a_title() {
        let r = result(serde_json::json!({
            "Name": { "type": "title", "title": [] }
        }));
        assert_eq!(title_of(&r), None);
    }
}
