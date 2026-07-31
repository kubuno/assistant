DROP INDEX IF EXISTS assistant.idx_assistant_conv_position;
ALTER TABLE assistant.conversations DROP COLUMN IF EXISTS position;
