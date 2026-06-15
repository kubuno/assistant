CREATE TABLE jarvis.provider_config (
    provider      VARCHAR(20) PRIMARY KEY
                      CHECK (provider IN ('ollama','openai','anthropic','google')),
    enabled       BOOLEAN NOT NULL DEFAULT FALSE,
    api_key       TEXT    NOT NULL DEFAULT '',
    base_url      TEXT    NOT NULL DEFAULT '',
    default_model VARCHAR(100) NOT NULL DEFAULT '',
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO jarvis.provider_config (provider, enabled, base_url, default_model) VALUES
    ('ollama',    true,  'http://localhost:11434', 'llama3.2:3b'),
    ('openai',    false, 'https://api.openai.com/v1', 'gpt-4o-mini'),
    ('anthropic', false, 'https://api.anthropic.com',  'claude-3-5-haiku-20241022'),
    ('google',    false, 'https://generativelanguage.googleapis.com', 'gemini-2.0-flash')
ON CONFLICT DO NOTHING;
