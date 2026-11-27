//! Postgres: the pool, the migration, and the handful of queries the portal
//! needs. Deliberately not a repository abstraction -- there are six.

use chrono::{DateTime, Duration, Utc};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;

pub type Result<T> = std::result::Result<T, sqlx::Error>;

/// How long a session is good for. Long enough that the portal is not a
/// nuisance, short enough that a stolen cookie is not forever.
const SESSION_TTL_DAYS: i64 = 30;

#[derive(Clone)]
pub struct Db {
    pool: PgPool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarLink {
    pub id: i64,
    pub display_name: String,
    /// The collection path. Shown to the user as a full URL, because some
    /// clients cannot discover a calendar and have to be handed one.
    pub calendar_uri: String,
    pub notion_parent_page: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotionConnection {
    pub id: i64,
    pub workspace_id: String,
    pub workspace_name: String,
    /// `None` is healthy. `Some` means Notion refused this token and the
    /// sync loop has stopped using it.
    pub token_invalid_since: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct Account {
    pub id: i64,
    pub email: String,
    pub caldav_username: Option<String>,
    pub caldav_password: Option<String>,
}

impl Db {
    pub async fn connect(url: &str) -> Result<Db> {
        let pool = PgPoolOptions::new().max_connections(8).connect(url).await?;
        Ok(Db { pool })
    }

    /// Idempotent, and serialised by an advisory lock.
    ///
    /// Both halves matter and neither is enough alone: `IF NOT EXISTS` does
    /// not make a concurrent loser wait, it lets two replicas proceed and
    /// then fails one on a duplicate pg_type entry.
    pub async fn migrate(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(0x6361_6c64_7061_7274_i64)
            .execute(&mut *tx)
            .await?;
        // Concatenated rather than run as separate transactions: together they
        // are one unit of "bring this database up to date", and a crash
        // between two of them would leave a shape no code expects.
        sqlx::raw_sql(concat!(
            include_str!("../migrations/0001_init.sql"),
            "\n",
            include_str!("../migrations/0002_notion.sql"),
            "\n",
            include_str!("../migrations/0003_links.sql"),
        ))
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }

    pub async fn upsert_account(&self, keycloak_sub: &str, email: &str) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO account (keycloak_sub, email) VALUES ($1, $2)
             ON CONFLICT (keycloak_sub) DO UPDATE SET email = EXCLUDED.email
             RETURNING id",
        )
        .bind(keycloak_sub)
        .bind(email)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("id"))
    }

    pub async fn account(&self, id: i64) -> Result<Option<Account>> {
        let row = sqlx::query(
            "SELECT id, email, caldav_username, caldav_password FROM account WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| Account {
            id: r.get("id"),
            email: r.get("email"),
            caldav_username: r.get("caldav_username"),
            caldav_password: r.get("caldav_password"),
        }))
    }

    /// Claim the right to provision this account's CalDAV credential.
    ///
    /// `WHERE caldav_username IS NULL` is the whole point: two tabs opening
    /// `/me` at once would otherwise both call the admin endpoint, and the
    /// second would overwrite the first's password here while caldav-rs kept
    /// the first one's hash. Exactly one caller gets `true`.
    pub async fn claim_credential(
        &self,
        account_id: i64,
        username: &str,
        password: &str,
    ) -> Result<bool> {
        let done = sqlx::query(
            "UPDATE account SET caldav_username = $2, caldav_password = $3
             WHERE id = $1 AND caldav_username IS NULL",
        )
        .bind(account_id)
        .bind(username)
        .bind(password)
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Record a rotation that caldav-rs has already accepted.
    pub async fn set_password(&self, account_id: i64, password: &str) -> Result<()> {
        sqlx::query("UPDATE account SET caldav_password = $2 WHERE id = $1")
            .bind(account_id)
            .bind(password)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn create_session(&self, id: &str, account_id: i64) -> Result<()> {
        let expires: DateTime<Utc> = Utc::now() + Duration::days(SESSION_TTL_DAYS);
        sqlx::query("INSERT INTO session (id, account_id, expires_at) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(account_id)
            .bind(expires)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Expiry is checked in the query, not after it. A session read that
    /// returns the row and leaves the caller to compare timestamps is a
    /// session that stays valid wherever someone forgets to.
    pub async fn session_account(&self, id: &str) -> Result<Option<i64>> {
        let row =
            sqlx::query("SELECT account_id FROM session WHERE id = $1 AND expires_at > now()")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.get("account_id")))
    }

    /// Backdates a session so the expiry path can be tested without sleeping
    /// for thirty days. Test-only, and compiled out of a release build.
    #[cfg(debug_assertions)]
    pub async fn expire_session_for_test(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE session SET expires_at = now() - interval '1 second' WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_session(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM session WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---- Links ----

    /// The token for one of this account's connections.
    ///
    /// Scoped by account in the query. Reading by connection id alone would
    /// hand anyone with a session someone else's Notion token.
    pub async fn connection_token(
        &self,
        account_id: i64,
        connection_id: i64,
    ) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT access_token FROM notion_connection WHERE id = $1 AND account_id = $2",
        )
        .bind(connection_id)
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.get("access_token")))
    }

    /// Reserves a link before the calendar exists.
    ///
    /// Inserted first, like `claim_credential`, and for the same reason: the
    /// UNIQUE on `(account_id, notion_parent_page)` is what stops two tabs
    /// from both creating a calendar for the same page, and a database is
    /// the only thing that can decide that. `ON CONFLICT DO NOTHING` makes
    /// "already linked" a value rather than an error to parse.
    pub async fn claim_link(
        &self,
        account_id: i64,
        connection_id: i64,
        parent_page: &str,
        display_name: &str,
        calendar_uri: &str,
    ) -> Result<Option<i64>> {
        let row = sqlx::query(
            "INSERT INTO calendar_link
               (account_id, notion_connection_id, notion_parent_page, display_name,
                caldav_calendar_uri)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT DO NOTHING
             RETURNING id",
        )
        .bind(account_id)
        .bind(connection_id)
        .bind(parent_page)
        .bind(display_name)
        .bind(calendar_uri)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.get("id")))
    }

    /// Undoes a claim whose calendar could not be created.
    ///
    /// Without this a failed MKCALENDAR would leave a row pointing at a
    /// collection that does not exist, and the UNIQUE constraint would then
    /// refuse every retry -- the user would be permanently unable to link a
    /// page because of one transient failure.
    pub async fn release_link(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM calendar_link WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn links(&self, account_id: i64) -> Result<Vec<CalendarLink>> {
        let rows = sqlx::query(
            "SELECT id, display_name, caldav_calendar_uri, notion_parent_page
             FROM calendar_link WHERE account_id = $1 ORDER BY created_at",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| CalendarLink {
                id: r.get("id"),
                display_name: r.get("display_name"),
                calendar_uri: r.get("caldav_calendar_uri"),
                notion_parent_page: r.get("notion_parent_page"),
            })
            .collect())
    }

    // ---- Notion connections ----

    pub async fn start_notion_flow(&self, state: &str, account_id: i64) -> Result<()> {
        sqlx::query("INSERT INTO notion_flow (state, account_id) VALUES ($1, $2)")
            .bind(state)
            .bind(account_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Consumes the flow, like `finish_login`: a replayed callback must find
    /// nothing rather than attach a second grant.
    pub async fn finish_notion_flow(&self, state: &str) -> Result<Option<i64>> {
        let row = sqlx::query(
            "DELETE FROM notion_flow
             WHERE state = $1 AND created_at > now() - interval '10 minutes'
             RETURNING account_id",
        )
        .bind(state)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.get("account_id")))
    }

    /// Records a grant, replacing any previous one for the same workspace.
    ///
    /// `token_invalid_since = NULL` in the update is the point: reconnecting
    /// is how a user fixes a revoked token, so it has to clear the mark that
    /// stopped the sync loop. Leaving it set would make a successful
    /// reconnect look identical to the broken state it was meant to repair.
    pub async fn put_notion_connection(
        &self,
        account_id: i64,
        token: &str,
        workspace_id: &str,
        workspace_name: &str,
        bot_id: &str,
    ) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO notion_connection
               (account_id, access_token, workspace_id, workspace_name, bot_id)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (account_id, workspace_id) DO UPDATE SET
               access_token        = EXCLUDED.access_token,
               workspace_name      = EXCLUDED.workspace_name,
               bot_id              = EXCLUDED.bot_id,
               token_invalid_since = NULL
             RETURNING id",
        )
        .bind(account_id)
        .bind(token)
        .bind(workspace_id)
        .bind(workspace_name)
        .bind(bot_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("id"))
    }

    pub async fn notion_connections(&self, account_id: i64) -> Result<Vec<NotionConnection>> {
        let rows = sqlx::query(
            "SELECT id, workspace_id, workspace_name, token_invalid_since
             FROM notion_connection WHERE account_id = $1 ORDER BY created_at",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| NotionConnection {
                id: r.get("id"),
                workspace_id: r.get("workspace_id"),
                workspace_name: r.get("workspace_name"),
                token_invalid_since: r.get("token_invalid_since"),
            })
            .collect())
    }

    /// Marks a grant as refused by Notion.
    ///
    /// Idempotent on the timestamp -- `WHERE token_invalid_since IS NULL` --
    /// so the recorded time is when it was *first* seen to fail, not when it
    /// was last retried. The difference matters for telling someone how long
    /// their calendar has been stale.
    pub async fn mark_notion_invalid(&self, connection_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE notion_connection SET token_invalid_since = now()
             WHERE id = $1 AND token_invalid_since IS NULL",
        )
        .bind(connection_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_notion_connection(&self, account_id: i64, id: i64) -> Result<()> {
        // Scoped by account, not just by id: an id alone would let anyone
        // with a session delete anyone else's connection.
        sqlx::query("DELETE FROM notion_connection WHERE id = $1 AND account_id = $2")
            .bind(id)
            .bind(account_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn start_login(&self, state: &str, verifier: &str, nonce: &str) -> Result<()> {
        sqlx::query("INSERT INTO login_flow (state, pkce_verifier, nonce) VALUES ($1, $2, $3)")
            .bind(state)
            .bind(verifier)
            .bind(nonce)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Consumes the flow: `DELETE ... RETURNING` so a replayed callback finds
    /// nothing. Leaving the row and deleting it afterwards would let the same
    /// authorization code be presented twice in the window between.
    ///
    /// The 10-minute bound is here rather than in a sweeper so that an old
    /// row cannot be used even if nothing has swept yet.
    pub async fn finish_login(&self, state: &str) -> Result<Option<(String, String)>> {
        let row = sqlx::query(
            "DELETE FROM login_flow
             WHERE state = $1 AND created_at > now() - interval '10 minutes'
             RETURNING pkce_verifier, nonce",
        )
        .bind(state)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| (r.get("pkce_verifier"), r.get("nonce"))))
    }
}
