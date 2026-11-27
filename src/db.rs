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
        sqlx::raw_sql(include_str!("../migrations/0001_init.sql"))
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
