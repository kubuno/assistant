-- Manual ordering of conversations (drag-and-drop in the sidebar).
-- Lower position = higher in its section. Defaults to 0 (ties fall back to
-- updated_at, preserving the previous recency order until the user reorders).
ALTER TABLE assistant.conversations
    ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_assistant_conv_position
    ON assistant.conversations(owner_id, position, updated_at DESC);
