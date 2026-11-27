-- One calendar bound to one Notion database.
--
-- The portal owns this table; the bridge reads it. What the bridge writes is
-- `notion_target`, keyed by the same `caldav_calendar_uri` -- the data source
-- id and the property ids, which only exist once the database has actually
-- been created in Notion. So the split is: this row says *where*, that row
-- says *what*, and neither service writes the other's.
CREATE TABLE IF NOT EXISTS calendar_link (
    id                   bigserial PRIMARY KEY,
    account_id           bigint NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    notion_connection_id bigint NOT NULL REFERENCES notion_connection(id) ON DELETE CASCADE,
    -- The page the calendar database is created under. Not a database id:
    -- the bridge creates the database itself from a fixed template, so the
    -- columns are known by id and a user renaming one does not break the
    -- mapping. Picking an existing database would mean guessing which column
    -- is the date.
    notion_parent_page   text NOT NULL,
    display_name         text NOT NULL DEFAULT '',
    -- The collection path in caldav-rs, e.g. `/calendars/u1/notion-1/`. This
    -- is the join key to everything the bridge stores.
    caldav_calendar_uri  text NOT NULL UNIQUE,
    created_at           timestamptz NOT NULL DEFAULT now(),
    -- One link per page per account. Linking the same page twice would have
    -- two calendars racing to own one Notion database.
    UNIQUE (account_id, notion_parent_page)
);

CREATE INDEX IF NOT EXISTS calendar_link_account_idx ON calendar_link (account_id);
