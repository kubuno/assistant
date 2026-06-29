DROP INDEX IF EXISTS jarvis.idx_jarvis_conv_position;
ALTER TABLE jarvis.conversations DROP COLUMN IF EXISTS position;
