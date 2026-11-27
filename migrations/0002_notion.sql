-- One Notion OAuth grant per connection.
--
-- Its own table rather than columns on `account`, because reconnecting to a
-- different workspace is a new grant and not an edit: the old token stops
-- working the moment the user revokes it, and overwriting in place would
-- lose which workspace the links under it belonged to.
CREATE TABLE IF NOT EXISTS notion_connection (
    id             bigserial PRIMARY KEY,
    account_id     bigint NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    -- Plaintext, consistent with `account.caldav_password` and recorded as a
    -- deliberate choice rather than an oversight. Unlike that one this is
    -- *not* a password the portal has to show anyone -- it is only ever sent
    -- to Notion -- so it is the column most worth revisiting first if the
    -- two-copy arrangement is ever tightened.
    access_token   text NOT NULL,
    workspace_id   text NOT NULL,
    workspace_name text NOT NULL DEFAULT '',
    bot_id         text NOT NULL DEFAULT '',
    -- NULL means healthy. Set when Notion answers 401 for this token, so the
    -- sync loop stops retrying a revoked grant and the owner can be told.
    -- Cleared by reconnecting.
    token_invalid_since timestamptz,
    created_at     timestamptz NOT NULL DEFAULT now(),
    -- One live grant per workspace per account. Reconnecting the same
    -- workspace replaces the token rather than accumulating rows nobody can
    -- tell apart.
    UNIQUE (account_id, workspace_id)
);

CREATE INDEX IF NOT EXISTS notion_connection_account_idx
    ON notion_connection (account_id);

-- The half of a Notion connect that has to survive the redirect.
--
-- Separate from `login_flow` because it carries something that one does not:
-- which account is connecting. Sign-in has no account yet -- that is what it
-- is establishing -- while this starts from a session and must come back to
-- the same one.
CREATE TABLE IF NOT EXISTS notion_flow (
    state      text PRIMARY KEY,
    account_id bigint NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now()
);
