-- Portable array column: PostgreSQL's `TEXT[]` has no equivalent on MySQL/SQLite,
-- so `enabled_tools` becomes a JSON array of strings — exactly what
-- `#[sqlx(json)] Vec<String>` / kubuno-db's `DbValue::Json` read and write.
-- `to_jsonb(text[])` yields `["tool", ...]`, so existing rows round-trip
-- unchanged. The default switches from the empty array literal `'{}'` (text[]) to
-- `'[]'::jsonb`.
ALTER TABLE assistant.agents
    ALTER COLUMN enabled_tools DROP DEFAULT,
    ALTER COLUMN enabled_tools TYPE jsonb USING to_jsonb(enabled_tools),
    ALTER COLUMN enabled_tools SET DEFAULT '[]'::jsonb;
