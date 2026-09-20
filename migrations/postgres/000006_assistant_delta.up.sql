-- Delta primitives for the local-first pull (conversations, folders, agents):
-- monotonic change_seq + tombstones. Messages bump their parent conversation.
-- NB: the existing stats trigger (AFTER INSERT ON messages → UPDATE conversations)
-- already fires the conversation BEFORE UPDATE below, so message inserts bump the
-- conversation for free; only message UPDATE/DELETE need an explicit child bump.

-- ===== conversations =====
CREATE SEQUENCE IF NOT EXISTS assistant.conv_change_seq;
ALTER TABLE assistant.conversations ADD COLUMN IF NOT EXISTS change_seq BIGINT NOT NULL DEFAULT nextval('assistant.conv_change_seq');
CREATE INDEX IF NOT EXISTS idx_assistant_conv_change_seq ON assistant.conversations(owner_id, change_seq);

CREATE OR REPLACE FUNCTION assistant.bump_conv_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('assistant.conv_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_conv_change_seq ON assistant.conversations;
CREATE TRIGGER trg_conv_change_seq BEFORE UPDATE ON assistant.conversations
    FOR EACH ROW EXECUTE FUNCTION assistant.bump_conv_change_seq();

CREATE TABLE IF NOT EXISTS assistant.conv_tombstones (
    id UUID PRIMARY KEY, owner_id UUID NOT NULL, change_seq BIGINT NOT NULL, deleted_at TIMESTAMPTZ NOT NULL DEFAULT NOW());
CREATE INDEX IF NOT EXISTS idx_assistant_conv_tomb ON assistant.conv_tombstones(owner_id, change_seq);
CREATE OR REPLACE FUNCTION assistant.conv_tombstone() RETURNS trigger AS $$
BEGIN
    INSERT INTO assistant.conv_tombstones (id, owner_id, change_seq)
    VALUES (OLD.id, OLD.owner_id, nextval('assistant.conv_change_seq'))
    ON CONFLICT (id) DO UPDATE SET change_seq = EXCLUDED.change_seq, deleted_at = NOW();
    RETURN OLD;
END; $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_conv_tombstone ON assistant.conversations;
CREATE TRIGGER trg_conv_tombstone AFTER DELETE ON assistant.conversations
    FOR EACH ROW EXECUTE FUNCTION assistant.conv_tombstone();

-- messages (UPDATE feedback / DELETE) bump their conversation
CREATE OR REPLACE FUNCTION assistant.msg_bump_conv() RETURNS trigger AS $$
BEGIN
    UPDATE assistant.conversations SET change_seq = change_seq
     WHERE id = COALESCE(NEW.conversation_id, OLD.conversation_id);
    RETURN COALESCE(NEW, OLD);
END; $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_msg_bump_conv ON assistant.messages;
CREATE TRIGGER trg_msg_bump_conv AFTER UPDATE OR DELETE ON assistant.messages
    FOR EACH ROW EXECUTE FUNCTION assistant.msg_bump_conv();

-- ===== folders =====
CREATE SEQUENCE IF NOT EXISTS assistant.folder_change_seq;
ALTER TABLE assistant.folders ADD COLUMN IF NOT EXISTS change_seq BIGINT NOT NULL DEFAULT nextval('assistant.folder_change_seq');
CREATE INDEX IF NOT EXISTS idx_assistant_folder_change_seq ON assistant.folders(owner_id, change_seq);
CREATE OR REPLACE FUNCTION assistant.bump_folder_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('assistant.folder_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_folder_change_seq ON assistant.folders;
CREATE TRIGGER trg_folder_change_seq BEFORE UPDATE ON assistant.folders
    FOR EACH ROW EXECUTE FUNCTION assistant.bump_folder_change_seq();
CREATE TABLE IF NOT EXISTS assistant.folder_tombstones (
    id UUID PRIMARY KEY, owner_id UUID NOT NULL, change_seq BIGINT NOT NULL, deleted_at TIMESTAMPTZ NOT NULL DEFAULT NOW());
CREATE INDEX IF NOT EXISTS idx_assistant_folder_tomb ON assistant.folder_tombstones(owner_id, change_seq);
CREATE OR REPLACE FUNCTION assistant.folder_tombstone() RETURNS trigger AS $$
BEGIN
    INSERT INTO assistant.folder_tombstones (id, owner_id, change_seq)
    VALUES (OLD.id, OLD.owner_id, nextval('assistant.folder_change_seq'))
    ON CONFLICT (id) DO UPDATE SET change_seq = EXCLUDED.change_seq, deleted_at = NOW();
    RETURN OLD;
END; $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_folder_tombstone ON assistant.folders;
CREATE TRIGGER trg_folder_tombstone AFTER DELETE ON assistant.folders
    FOR EACH ROW EXECUTE FUNCTION assistant.folder_tombstone();

-- ===== agents (owner NULL for system agents) =====
CREATE SEQUENCE IF NOT EXISTS assistant.agent_change_seq;
ALTER TABLE assistant.agents ADD COLUMN IF NOT EXISTS change_seq BIGINT NOT NULL DEFAULT nextval('assistant.agent_change_seq');
CREATE INDEX IF NOT EXISTS idx_assistant_agent_change_seq ON assistant.agents(change_seq);
CREATE OR REPLACE FUNCTION assistant.bump_agent_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('assistant.agent_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_agent_change_seq ON assistant.agents;
CREATE TRIGGER trg_agent_change_seq BEFORE UPDATE ON assistant.agents
    FOR EACH ROW EXECUTE FUNCTION assistant.bump_agent_change_seq();
CREATE TABLE IF NOT EXISTS assistant.agent_tombstones (
    id UUID PRIMARY KEY, owner_id UUID, change_seq BIGINT NOT NULL, deleted_at TIMESTAMPTZ NOT NULL DEFAULT NOW());
CREATE INDEX IF NOT EXISTS idx_assistant_agent_tomb ON assistant.agent_tombstones(change_seq);
CREATE OR REPLACE FUNCTION assistant.agent_tombstone() RETURNS trigger AS $$
BEGIN
    INSERT INTO assistant.agent_tombstones (id, owner_id, change_seq)
    VALUES (OLD.id, OLD.owner_id, nextval('assistant.agent_change_seq'))
    ON CONFLICT (id) DO UPDATE SET change_seq = EXCLUDED.change_seq, deleted_at = NOW();
    RETURN OLD;
END; $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_agent_tombstone ON assistant.agents;
CREATE TRIGGER trg_agent_tombstone AFTER DELETE ON assistant.agents
    FOR EACH ROW EXECUTE FUNCTION assistant.agent_tombstone();
