-- `UNIQUE (account_id, notion_parent_page)` guarded the page picker: it
-- stopped two calendars racing to own one page. Notion duplicates a database
-- during consent now, so that column is NULL on every row and the constraint
-- guards nothing -- Postgres allows any number of NULLs in a unique index.
--
-- Dropped rather than left: a constraint that cannot fire still has to be
-- read and understood by whoever comes next, and this one describes a flow
-- that no longer exists.
ALTER TABLE calendar_link DROP CONSTRAINT IF EXISTS calendar_link_account_id_notion_parent_page_key;
