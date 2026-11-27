//! What the handlers and server functions need, in one place.

use crate::caldav_admin::CaldavAdmin;
use crate::db::Db;
use crate::notion_oauth::NotionOauth;
use crate::oidc::Oidc;

#[derive(Clone)]
pub struct ServerState {
    pub db: Db,
    pub oidc: Oidc,
    pub caldav: CaldavAdmin,
    pub notion: NotionOauth,
    /// The CalDAV URL shown to a person to type into their phone. Not derived
    /// from the admin URL: that one points at a port on a loopback or cluster
    /// address, and is never reachable from a phone.
    pub caldav_public_url: String,
    /// Whether to mark the session cookie `Secure`. False over plain HTTP in
    /// development, where marking it would have the browser drop it and the
    /// login would silently never stick.
    pub secure_cookies: bool,
}
