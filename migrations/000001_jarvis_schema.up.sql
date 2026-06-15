CREATE SCHEMA IF NOT EXISTS jarvis;

CREATE OR REPLACE FUNCTION jarvis.set_updated_at()
RETURNS TRIGGER AS $$
BEGIN NEW.updated_at = NOW(); RETURN NEW; END;
$$ LANGUAGE plpgsql;

-- =====================
-- CONVERSATIONS
-- =====================
CREATE TABLE jarvis.conversations (
    id               UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id         UUID NOT NULL,
    title            VARCHAR(500),
    agent_id         UUID,
    model_id         VARCHAR(100) NOT NULL DEFAULT 'llama3.2:3b',
    provider         VARCHAR(20)  NOT NULL DEFAULT 'ollama'
                         CHECK (provider IN ('ollama','openai','anthropic','google')),
    memory_summary   TEXT,
    generation_params JSONB NOT NULL DEFAULT '{"temperature":0.7,"top_p":0.9,"max_tokens":4096}',
    is_pinned        BOOLEAN NOT NULL DEFAULT FALSE,
    is_archived      BOOLEAN NOT NULL DEFAULT FALSE,
    is_trashed       BOOLEAN NOT NULL DEFAULT FALSE,
    message_count    INTEGER NOT NULL DEFAULT 0,
    total_tokens     INTEGER NOT NULL DEFAULT 0,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_jarvis_conv_owner  ON jarvis.conversations(owner_id, updated_at DESC);
CREATE INDEX idx_jarvis_conv_pinned ON jarvis.conversations(owner_id) WHERE is_pinned = TRUE;

CREATE TRIGGER conversations_updated_at
    BEFORE UPDATE ON jarvis.conversations
    FOR EACH ROW EXECUTE FUNCTION jarvis.set_updated_at();

-- =====================
-- MESSAGES
-- =====================
CREATE TABLE jarvis.messages (
    id                UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    conversation_id   UUID NOT NULL REFERENCES jarvis.conversations(id) ON DELETE CASCADE,
    role              VARCHAR(15) NOT NULL
                          CHECK (role IN ('user', 'assistant', 'system', 'tool')),
    content           TEXT NOT NULL DEFAULT '',
    attachments       JSONB NOT NULL DEFAULT '[]',
    tool_calls        JSONB NOT NULL DEFAULT '[]',
    rag_sources       JSONB NOT NULL DEFAULT '[]',
    prompt_tokens     INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    generation_ms     INTEGER,
    feedback          VARCHAR(10) CHECK (feedback IN ('like', 'dislike')),
    is_regenerated    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_jarvis_messages_conv ON jarvis.messages(conversation_id, created_at ASC);

-- Trigger: incrémenter message_count
CREATE OR REPLACE FUNCTION jarvis.update_conversation_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE jarvis.conversations
    SET message_count = message_count + 1,
        total_tokens  = total_tokens + NEW.prompt_tokens + NEW.completion_tokens,
        updated_at    = NOW()
    WHERE id = NEW.conversation_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER messages_update_conv
    AFTER INSERT ON jarvis.messages
    FOR EACH ROW EXECUTE FUNCTION jarvis.update_conversation_stats();
