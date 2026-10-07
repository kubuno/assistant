-- Back to PostgreSQL's native TEXT[] from the JSON array of strings.
ALTER TABLE assistant.agents
    ALTER COLUMN enabled_tools DROP DEFAULT,
    ALTER COLUMN enabled_tools TYPE text[]
        USING (SELECT COALESCE(array_agg(e), '{}')
                 FROM jsonb_array_elements_text(enabled_tools) AS e),
    ALTER COLUMN enabled_tools SET DEFAULT '{}';
