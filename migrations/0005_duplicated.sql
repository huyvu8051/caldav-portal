-- The database Notion duplicated into the workspace during consent.
--
-- This is what makes the picker unnecessary. The calendar database arrives
-- with the grant, already shaped the way the bridge expects, so there is
-- nothing to choose and no window where the portal has reported success but
-- no database exists.
--
-- Nullable: a user can still press "select existing pages" on Notion's
-- consent screen instead of duplicating, and then there is no database and
-- no link to make.
ALTER TABLE calendar_link ADD COLUMN IF NOT EXISTS notion_database_id text;

-- The parent page stops being required, since a duplicated database has no
-- page of ours above it.
ALTER TABLE calendar_link ALTER COLUMN notion_parent_page DROP NOT NULL;

-- One link per duplicated database, so a replayed callback cannot make two.
CREATE UNIQUE INDEX IF NOT EXISTS calendar_link_database_key
    ON calendar_link (account_id, notion_database_id);
