-- Why someone is being sent to Notion.
--
-- "Add a calendar" and "Reconnect" are the same OAuth round trip and must
-- not have the same result. Notion duplicates the template on every consent,
-- so a reconnect -- whose entire purpose is to replace a dead token --
-- would come back with a fresh database id and quietly add a second
-- calendar. Fixing a connection would grow the list.
--
-- Defaulted, so rows written before this exists read as the common case.
ALTER TABLE notion_flow ADD COLUMN IF NOT EXISTS intent text NOT NULL DEFAULT 'add';
