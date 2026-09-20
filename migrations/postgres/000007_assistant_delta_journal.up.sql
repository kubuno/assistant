-- Move the delta layer off PostgreSQL sequences + triggers and onto the
-- application-driven `kubuno_db::journal` primitive (one shared counter row per
-- domain, seqs taken in Rust at write time, tombstones written in the same
-- transaction). Neither the sequence nor the trigger mechanism has a portable
-- form on MySQL/SQLite, so it is retired here on PostgreSQL too; the tombstone
-- TABLES keep their exact shape (no data migration), only their triggers go.
--
-- The `change_seq` columns stay `BIGINT NOT NULL`, but their DEFAULT switches
-- from `nextval(...)` to `0`: the application now supplies every value.
--
-- The message-stats trigger (`AFTER INSERT ON messages`) is also retired: it
-- maintained `message_count` / `total_tokens` in the database, has no portable
-- form, and — through the conversation UPDATE it issued — used to be what bumped
-- the conversation's `change_seq` on a new message. All of that now happens in
-- Rust at the message write site. The `conversations` `updated_at` trigger from
-- 000001 is deliberately LEFT in place (its MySQL/SQLite equivalents are
-- declared in those engines' migrations).

-- ── conversations: change-seq bump, tombstone, and the child (message) bumps ──
DROP TRIGGER IF EXISTS trg_conv_change_seq ON assistant.conversations;
DROP TRIGGER IF EXISTS trg_conv_tombstone  ON assistant.conversations;
DROP TRIGGER IF EXISTS trg_msg_bump_conv   ON assistant.messages;
DROP TRIGGER IF EXISTS messages_update_conv ON assistant.messages;
DROP FUNCTION IF EXISTS assistant.bump_conv_change_seq();
DROP FUNCTION IF EXISTS assistant.conv_tombstone();
DROP FUNCTION IF EXISTS assistant.msg_bump_conv();
DROP FUNCTION IF EXISTS assistant.update_conversation_stats();

-- ── folders ──────────────────────────────────────────────────────────────────
DROP TRIGGER IF EXISTS trg_folder_change_seq ON assistant.folders;
DROP TRIGGER IF EXISTS trg_folder_tombstone  ON assistant.folders;
DROP FUNCTION IF EXISTS assistant.bump_folder_change_seq();
DROP FUNCTION IF EXISTS assistant.folder_tombstone();

-- ── agents ───────────────────────────────────────────────────────────────────
DROP TRIGGER IF EXISTS trg_agent_change_seq ON assistant.agents;
DROP TRIGGER IF EXISTS trg_agent_tombstone  ON assistant.agents;
DROP FUNCTION IF EXISTS assistant.bump_agent_change_seq();
DROP FUNCTION IF EXISTS assistant.agent_tombstone();

-- The DEFAULT references the sequence, so it must go before the sequence does.
ALTER TABLE assistant.conversations ALTER COLUMN change_seq SET DEFAULT 0;
ALTER TABLE assistant.folders       ALTER COLUMN change_seq SET DEFAULT 0;
ALTER TABLE assistant.agents        ALTER COLUMN change_seq SET DEFAULT 0;
DROP SEQUENCE IF EXISTS assistant.conv_change_seq;
DROP SEQUENCE IF EXISTS assistant.folder_change_seq;
DROP SEQUENCE IF EXISTS assistant.agent_change_seq;

-- ── The journal's shared counter, seeded to continue the existing sequences ───

CREATE TABLE IF NOT EXISTS assistant.change_counter (
    domain VARCHAR(190) NOT NULL PRIMARY KEY,
    n      BIGINT       NOT NULL
);

-- Seed each domain to the current max so `next_seq` (n := n + 1) never hands out
-- a value an existing row already holds.
INSERT INTO assistant.change_counter (domain, n)
    SELECT 'conversations', COALESCE(MAX(change_seq), 0) FROM assistant.conversations
    ON CONFLICT (domain) DO NOTHING;
INSERT INTO assistant.change_counter (domain, n)
    SELECT 'folders', COALESCE(MAX(change_seq), 0) FROM assistant.folders
    ON CONFLICT (domain) DO NOTHING;
INSERT INTO assistant.change_counter (domain, n)
    SELECT 'agents', COALESCE(MAX(change_seq), 0) FROM assistant.agents
    ON CONFLICT (domain) DO NOTHING;

-- The tombstone tables (assistant.conv_tombstones, folder_tombstones,
-- agent_tombstones) keep their 000006 shape unchanged; only their triggers were
-- dropped above.
