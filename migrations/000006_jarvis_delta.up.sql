-- Delta primitives for the local-first pull (conversations, folders, agents):
-- monotonic change_seq + tombstones. Messages bump their parent conversation.
-- NB: the existing stats trigger (AFTER INSERT ON messages → UPDATE conversations)
-- already fires the conversation BEFORE UPDATE below, so message inserts bump the
-- conversation for free; only message UPDATE/DELETE need an explicit child bump.

-- ===== conversations =====
CREATE SEQUENCE IF NOT EXISTS jarvis.conv_change_seq;
ALTER TABLE jarvis.conversations ADD COLUMN IF NOT EXISTS change_seq BIGINT NOT NULL DEFAULT nextval('jarvis.conv_change_seq');
CREATE INDEX IF NOT EXISTS idx_jarvis_conv_change_seq ON jarvis.conversations(owner_id, change_seq);

CREATE OR REPLACE FUNCTION jarvis.bump_conv_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('jarvis.conv_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_conv_change_seq ON jarvis.conversations;
CREATE TRIGGER trg_conv_change_seq BEFORE UPDATE ON jarvis.conversations
    FOR EACH ROW EXECUTE FUNCTION jarvis.bump_conv_change_seq();

CREATE TABLE IF NOT EXISTS jarvis.conv_tombstones (
    id UUID PRIMARY KEY, owner_id UUID NOT NULL, change_seq BIGINT NOT NULL, deleted_at TIMESTAMPTZ NOT NULL DEFAULT NOW());
CREATE INDEX IF NOT EXISTS idx_jarvis_conv_tomb ON jarvis.conv_tombstones(owner_id, change_seq);
CREATE OR REPLACE FUNCTION jarvis.conv_tombstone() RETURNS trigger AS $$
BEGIN
    INSERT INTO jarvis.conv_tombstones (id, owner_id, change_seq)
    VALUES (OLD.id, OLD.owner_id, nextval('jarvis.conv_change_seq'))
    ON CONFLICT (id) DO UPDATE SET change_seq = EXCLUDED.change_seq, deleted_at = NOW();
    RETURN OLD;
END; $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_conv_tombstone ON jarvis.conversations;
CREATE TRIGGER trg_conv_tombstone AFTER DELETE ON jarvis.conversations
    FOR EACH ROW EXECUTE FUNCTION jarvis.conv_tombstone();

-- messages (UPDATE feedback / DELETE) bump their conversation
CREATE OR REPLACE FUNCTION jarvis.msg_bump_conv() RETURNS trigger AS $$
BEGIN
    UPDATE jarvis.conversations SET change_seq = change_seq
     WHERE id = COALESCE(NEW.conversation_id, OLD.conversation_id);
    RETURN COALESCE(NEW, OLD);
END; $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_msg_bump_conv ON jarvis.messages;
CREATE TRIGGER trg_msg_bump_conv AFTER UPDATE OR DELETE ON jarvis.messages
    FOR EACH ROW EXECUTE FUNCTION jarvis.msg_bump_conv();

-- ===== folders =====
CREATE SEQUENCE IF NOT EXISTS jarvis.folder_change_seq;
ALTER TABLE jarvis.folders ADD COLUMN IF NOT EXISTS change_seq BIGINT NOT NULL DEFAULT nextval('jarvis.folder_change_seq');
CREATE INDEX IF NOT EXISTS idx_jarvis_folder_change_seq ON jarvis.folders(owner_id, change_seq);
CREATE OR REPLACE FUNCTION jarvis.bump_folder_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('jarvis.folder_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_folder_change_seq ON jarvis.folders;
CREATE TRIGGER trg_folder_change_seq BEFORE UPDATE ON jarvis.folders
    FOR EACH ROW EXECUTE FUNCTION jarvis.bump_folder_change_seq();
CREATE TABLE IF NOT EXISTS jarvis.folder_tombstones (
    id UUID PRIMARY KEY, owner_id UUID NOT NULL, change_seq BIGINT NOT NULL, deleted_at TIMESTAMPTZ NOT NULL DEFAULT NOW());
CREATE INDEX IF NOT EXISTS idx_jarvis_folder_tomb ON jarvis.folder_tombstones(owner_id, change_seq);
CREATE OR REPLACE FUNCTION jarvis.folder_tombstone() RETURNS trigger AS $$
BEGIN
    INSERT INTO jarvis.folder_tombstones (id, owner_id, change_seq)
    VALUES (OLD.id, OLD.owner_id, nextval('jarvis.folder_change_seq'))
    ON CONFLICT (id) DO UPDATE SET change_seq = EXCLUDED.change_seq, deleted_at = NOW();
    RETURN OLD;
END; $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_folder_tombstone ON jarvis.folders;
CREATE TRIGGER trg_folder_tombstone AFTER DELETE ON jarvis.folders
    FOR EACH ROW EXECUTE FUNCTION jarvis.folder_tombstone();

-- ===== agents (owner NULL for system agents) =====
CREATE SEQUENCE IF NOT EXISTS jarvis.agent_change_seq;
ALTER TABLE jarvis.agents ADD COLUMN IF NOT EXISTS change_seq BIGINT NOT NULL DEFAULT nextval('jarvis.agent_change_seq');
CREATE INDEX IF NOT EXISTS idx_jarvis_agent_change_seq ON jarvis.agents(change_seq);
CREATE OR REPLACE FUNCTION jarvis.bump_agent_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('jarvis.agent_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_agent_change_seq ON jarvis.agents;
CREATE TRIGGER trg_agent_change_seq BEFORE UPDATE ON jarvis.agents
    FOR EACH ROW EXECUTE FUNCTION jarvis.bump_agent_change_seq();
CREATE TABLE IF NOT EXISTS jarvis.agent_tombstones (
    id UUID PRIMARY KEY, owner_id UUID, change_seq BIGINT NOT NULL, deleted_at TIMESTAMPTZ NOT NULL DEFAULT NOW());
CREATE INDEX IF NOT EXISTS idx_jarvis_agent_tomb ON jarvis.agent_tombstones(change_seq);
CREATE OR REPLACE FUNCTION jarvis.agent_tombstone() RETURNS trigger AS $$
BEGIN
    INSERT INTO jarvis.agent_tombstones (id, owner_id, change_seq)
    VALUES (OLD.id, OLD.owner_id, nextval('jarvis.agent_change_seq'))
    ON CONFLICT (id) DO UPDATE SET change_seq = EXCLUDED.change_seq, deleted_at = NOW();
    RETURN OLD;
END; $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_agent_tombstone ON jarvis.agents;
CREATE TRIGGER trg_agent_tombstone AFTER DELETE ON jarvis.agents
    FOR EACH ROW EXECUTE FUNCTION jarvis.agent_tombstone();
