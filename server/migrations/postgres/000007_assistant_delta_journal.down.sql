-- Restore the sequence + trigger delta machinery of 000006 and the message-stats
-- trigger of 000001, and drop the journal counter. (The tombstone tables were
-- never dropped, so they are reused as-is.)

DROP TABLE IF EXISTS assistant.change_counter;

-- ── conversations ────────────────────────────────────────────────────────────
CREATE SEQUENCE IF NOT EXISTS assistant.conv_change_seq;
ALTER TABLE assistant.conversations ALTER COLUMN change_seq SET DEFAULT nextval('assistant.conv_change_seq');

CREATE OR REPLACE FUNCTION assistant.bump_conv_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('assistant.conv_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER trg_conv_change_seq BEFORE UPDATE ON assistant.conversations
    FOR EACH ROW EXECUTE FUNCTION assistant.bump_conv_change_seq();

CREATE OR REPLACE FUNCTION assistant.conv_tombstone() RETURNS trigger AS $$
BEGIN
    INSERT INTO assistant.conv_tombstones (id, owner_id, change_seq)
    VALUES (OLD.id, OLD.owner_id, nextval('assistant.conv_change_seq'))
    ON CONFLICT (id) DO UPDATE SET change_seq = EXCLUDED.change_seq, deleted_at = NOW();
    RETURN OLD;
END; $$ LANGUAGE plpgsql;
CREATE TRIGGER trg_conv_tombstone AFTER DELETE ON assistant.conversations
    FOR EACH ROW EXECUTE FUNCTION assistant.conv_tombstone();

CREATE OR REPLACE FUNCTION assistant.msg_bump_conv() RETURNS trigger AS $$
BEGIN
    UPDATE assistant.conversations SET change_seq = change_seq
     WHERE id = COALESCE(NEW.conversation_id, OLD.conversation_id);
    RETURN COALESCE(NEW, OLD);
END; $$ LANGUAGE plpgsql;
CREATE TRIGGER trg_msg_bump_conv AFTER UPDATE OR DELETE ON assistant.messages
    FOR EACH ROW EXECUTE FUNCTION assistant.msg_bump_conv();

CREATE OR REPLACE FUNCTION assistant.update_conversation_stats() RETURNS TRIGGER AS $$
BEGIN
    UPDATE assistant.conversations
    SET message_count = message_count + 1,
        total_tokens  = total_tokens + NEW.prompt_tokens + NEW.completion_tokens,
        updated_at    = NOW()
    WHERE id = NEW.conversation_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER messages_update_conv AFTER INSERT ON assistant.messages
    FOR EACH ROW EXECUTE FUNCTION assistant.update_conversation_stats();

-- ── folders ──────────────────────────────────────────────────────────────────
CREATE SEQUENCE IF NOT EXISTS assistant.folder_change_seq;
ALTER TABLE assistant.folders ALTER COLUMN change_seq SET DEFAULT nextval('assistant.folder_change_seq');
CREATE OR REPLACE FUNCTION assistant.bump_folder_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('assistant.folder_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER trg_folder_change_seq BEFORE UPDATE ON assistant.folders
    FOR EACH ROW EXECUTE FUNCTION assistant.bump_folder_change_seq();
CREATE OR REPLACE FUNCTION assistant.folder_tombstone() RETURNS trigger AS $$
BEGIN
    INSERT INTO assistant.folder_tombstones (id, owner_id, change_seq)
    VALUES (OLD.id, OLD.owner_id, nextval('assistant.folder_change_seq'))
    ON CONFLICT (id) DO UPDATE SET change_seq = EXCLUDED.change_seq, deleted_at = NOW();
    RETURN OLD;
END; $$ LANGUAGE plpgsql;
CREATE TRIGGER trg_folder_tombstone AFTER DELETE ON assistant.folders
    FOR EACH ROW EXECUTE FUNCTION assistant.folder_tombstone();

-- ── agents ───────────────────────────────────────────────────────────────────
CREATE SEQUENCE IF NOT EXISTS assistant.agent_change_seq;
ALTER TABLE assistant.agents ALTER COLUMN change_seq SET DEFAULT nextval('assistant.agent_change_seq');
CREATE OR REPLACE FUNCTION assistant.bump_agent_change_seq() RETURNS trigger AS $$
BEGIN NEW.change_seq := nextval('assistant.agent_change_seq'); RETURN NEW; END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER trg_agent_change_seq BEFORE UPDATE ON assistant.agents
    FOR EACH ROW EXECUTE FUNCTION assistant.bump_agent_change_seq();
CREATE OR REPLACE FUNCTION assistant.agent_tombstone() RETURNS trigger AS $$
BEGIN
    INSERT INTO assistant.agent_tombstones (id, owner_id, change_seq)
    VALUES (OLD.id, OLD.owner_id, nextval('assistant.agent_change_seq'))
    ON CONFLICT (id) DO UPDATE SET change_seq = EXCLUDED.change_seq, deleted_at = NOW();
    RETURN OLD;
END; $$ LANGUAGE plpgsql;
CREATE TRIGGER trg_agent_tombstone AFTER DELETE ON assistant.agents
    FOR EACH ROW EXECUTE FUNCTION assistant.agent_tombstone();
