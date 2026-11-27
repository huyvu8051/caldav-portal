-- Runs in full on every process start, so every statement is written to be a
-- no-op the second time. That is load-bearing, not tidiness: caldav-rs
-- shipped its first migration without `IF NOT EXISTS` and crash-looped on
-- deployment, dying on `relation already exists` before it could serve.
--
-- The caller runs this inside a transaction holding an advisory lock, which
-- is the other half: `CREATE TABLE IF NOT EXISTS` does not make a concurrent
-- loser wait, it lets both proceed and fails one on a duplicate pg_type row.

-- One row per person, keyed by Keycloak's `sub`.
--
-- The realm is shared with notion-caldav-saas on purpose, so this `sub` is
-- the same value that product already stores. That is what makes the
-- eventual migration a join rather than a manual match on email addresses.
CREATE TABLE IF NOT EXISTS account (
    id              bigserial PRIMARY KEY,
    keycloak_sub    text NOT NULL UNIQUE,
    email           text NOT NULL DEFAULT '',
    -- The account-level CalDAV login: one credential that sees every calendar
    -- this account links. Per-link credentials come later (A8).
    caldav_username text UNIQUE,
    -- Plaintext, deliberately, and named for what it is.
    --
    -- The portal has to be able to show this password again, and a hash does
    -- not come back. caldav-rs separately holds an argon2 hash of the same
    -- value -- that is the copy that actually verifies a login. So the
    -- password exists in two places: this one is the source, that one is
    -- derived from it.
    --
    -- They can only drift one way: running `caldav-rs adduser` by hand with a
    -- new password changes the hash there and tells nobody, and this column
    -- goes on displaying a password that no longer works. There is a warning
    -- beside that command for the same reason.
    --
    -- notion-caldav-saas made the same choice and then made it worse: it has
    -- a `caldav_password_encrypted` column that no code reads or writes, and
    -- a plaintext one beside it. A column claiming encryption that holds none
    -- is worse than either option taken honestly.
    caldav_password text,
    created_at      timestamptz NOT NULL DEFAULT now()
);

-- Server-side sessions. The cookie carries only this id, which is 256 random
-- bits: nothing about the account is in the cookie, so nothing in the cookie
-- has to be signed or trusted.
CREATE TABLE IF NOT EXISTS session (
    id         text PRIMARY KEY,
    account_id bigint NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL
);

CREATE INDEX IF NOT EXISTS session_expires_idx ON session (expires_at);

-- The half of an OIDC login that has to survive the redirect to Keycloak and
-- back: the PKCE verifier and the nonce.
--
-- Server-side rather than in a cookie. A cookie would work, but it would have
-- to be signed to stop a tampered verifier being handed back, and "store the
-- secret where the attacker can hold it, then prove they did not change it"
-- is more moving parts than one short-lived row.
CREATE TABLE IF NOT EXISTS login_flow (
    state         text PRIMARY KEY,
    pkce_verifier text NOT NULL,
    nonce         text NOT NULL,
    created_at    timestamptz NOT NULL DEFAULT now()
);
