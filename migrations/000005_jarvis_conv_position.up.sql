-- Manual ordering of conversations (drag-and-drop in the sidebar).
-- Lower position = higher in its section. Defaults to 0 (ties fall back to
-- updated_at, preserving the previous recency order until the user reorders).
ALTER TABLE jarvis.conversations
    ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_jarvis_conv_position
    ON jarvis.conversations(owner_id, position, updated_at DESC);
