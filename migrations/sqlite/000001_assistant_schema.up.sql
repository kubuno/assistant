-- SQLite — `assistant` is an ATTACHed database file, attached on every pooled
-- connection by kubuno-db, so the qualified names below resolve as they do on
-- the other two engines. This single file declares the FINAL shape the
-- PostgreSQL side reached across 000001..000008.
--
-- Differences from PostgreSQL, and why:
--   * UUID -> BLOB, TIMESTAMPTZ -> TEXT (`%F %T%.f`, UTC), JSONB / TEXT[] -> TEXT
--     (a JSON array/object), all as sqlx encodes/decodes them on SQLite.
--   * No DEFAULT on `id`: SQLite has no UUID generator; the process supplies it.
--   * The conversations `updated_at` is maintained by a hand-written AFTER UPDATE
--     trigger (SQLite has no ON UPDATE clause). It does not recurse: SQLite
--     leaves recursive_triggers off.
--   * `message_count` / `total_tokens` are maintained by the application, not a
--     trigger (there is no portable multi-statement trigger for it). Same for the
--     delta layer: it is the journal (change_counter + per-row change_seq).
--   * Partial indexes ARE kept (SQLite supports them).
--   * Foreign-key REFERENCES are unqualified (SQLite assumes the same database).

CREATE TABLE assistant.conversations (
    id                BLOB    NOT NULL PRIMARY KEY,
    owner_id          BLOB    NOT NULL,
    title             TEXT,
    agent_id          BLOB,
    model_id          TEXT    NOT NULL DEFAULT 'llama3.2:3b',
    provider          TEXT    NOT NULL DEFAULT 'ollama'
                          CHECK (provider IN ('ollama','openai','anthropic','google')),
    memory_summary    TEXT,
    generation_params TEXT    NOT NULL DEFAULT '{"temperature":0.7,"top_p":0.9,"max_tokens":4096}',
    is_pinned         INTEGER NOT NULL DEFAULT 0,
    is_archived       INTEGER NOT NULL DEFAULT 0,
    is_trashed        INTEGER NOT NULL DEFAULT 0,
    message_count     INTEGER NOT NULL DEFAULT 0,
    total_tokens      INTEGER NOT NULL DEFAULT 0,
    folder_id         BLOB    REFERENCES folders(id) ON DELETE SET NULL,
    position          INTEGER NOT NULL DEFAULT 0,
    change_seq        INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    updated_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
);
CREATE INDEX assistant.idx_assistant_conv_owner    ON conversations(owner_id, updated_at DESC);
CREATE INDEX assistant.idx_assistant_conv_pinned   ON conversations(owner_id) WHERE is_pinned = 1;
CREATE INDEX assistant.idx_assistant_conv_folder   ON conversations(folder_id);
CREATE INDEX assistant.idx_assistant_conv_position ON conversations(owner_id, position, updated_at DESC);
CREATE INDEX assistant.idx_assistant_conv_change_seq ON conversations(owner_id, change_seq);

CREATE TRIGGER assistant.conversations_updated_at AFTER UPDATE ON conversations
BEGIN
    UPDATE conversations SET updated_at = strftime('%Y-%m-%d %H:%M:%f', 'now') WHERE id = NEW.id;
END;

CREATE TABLE assistant.messages (
    id                BLOB    NOT NULL PRIMARY KEY,
    conversation_id   BLOB    NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role              TEXT    NOT NULL CHECK (role IN ('user', 'assistant', 'system', 'tool')),
    content           TEXT    NOT NULL DEFAULT '',
    attachments       TEXT    NOT NULL DEFAULT '[]',
    tool_calls        TEXT    NOT NULL DEFAULT '[]',
    rag_sources       TEXT    NOT NULL DEFAULT '[]',
    prompt_tokens     INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    generation_ms     INTEGER,
    feedback          TEXT    CHECK (feedback IN ('like', 'dislike')),
    is_regenerated    INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
);
CREATE INDEX assistant.idx_assistant_messages_conv ON messages(conversation_id, created_at ASC);

CREATE TABLE assistant.agents (
    id                 BLOB    NOT NULL PRIMARY KEY,
    owner_id           BLOB,
    name               TEXT    NOT NULL,
    description        TEXT,
    avatar_emoji       TEXT    NOT NULL DEFAULT '🤖',
    avatar_color       TEXT    NOT NULL DEFAULT '#1a73e8',
    system_prompt      TEXT    NOT NULL DEFAULT '',
    preferred_model    TEXT,
    preferred_provider TEXT,
    generation_params  TEXT    NOT NULL DEFAULT '{}',
    enabled_tools      TEXT    NOT NULL DEFAULT '[]',
    prompt_suggestions TEXT    NOT NULL DEFAULT '[]',
    is_public          INTEGER NOT NULL DEFAULT 0,
    is_system          INTEGER NOT NULL DEFAULT 0,
    change_seq         INTEGER NOT NULL DEFAULT 0,
    created_at         TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    updated_at         TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
);
CREATE INDEX assistant.idx_assistant_agents_owner  ON agents(owner_id);
CREATE INDEX assistant.idx_assistant_agents_public ON agents(is_public) WHERE is_public = 1;
CREATE INDEX assistant.idx_assistant_agents_system ON agents(is_system) WHERE is_system = 1;
CREATE INDEX assistant.idx_assistant_agent_change_seq ON agents(change_seq);

INSERT INTO assistant.agents (id, name, description, avatar_emoji, avatar_color, system_prompt, enabled_tools, is_system, prompt_suggestions) VALUES
(x'493a9330064e4ec9b1525dadfece6f2c', 'Assistant',
 'Assistant général Kubuno — accès à tous vos modules', '🤖', '#1a73e8',
 'Tu es Assistant, l''assistant IA de Kubuno, une plateforme cloud self-hosted. Tu es toujours utile, concis et précis. Réponds en français sauf si l''utilisateur écrit dans une autre langue.',
 '[]', 1,
 '[{"label": "Résume mes notes récentes", "prompt": "Résume les notes que j''ai créées cette semaine", "icon": "📝"},{"label": "Explique-moi…", "prompt": "Explique-moi en détail : ", "icon": "💡"},{"label": "Rédiger un email", "prompt": "Aide-moi à rédiger un email professionnel sur : ", "icon": "✉️"},{"label": "Analyser du code", "prompt": "Analyse et explique ce code : ", "icon": "💻"}]'),
(x'9d2037dd899643fdaf240e41748aa666', 'Expert Code',
 'Développeur senior — explique, débogue et génère du code dans tous les langages', '💻', '#e8824a',
 'Tu es un expert en développement logiciel. Tu aides à écrire, déboguer et expliquer du code dans tous les langages. Préfère les solutions simples et bien documentées. Utilise toujours des blocs de code avec la syntaxe appropriée.',
 '[]', 1,
 '[{"label": "Déboguer ce code", "prompt": "Voici du code qui ne fonctionne pas : ", "icon": "🐛"},{"label": "Expliquer ligne par ligne", "prompt": "Explique ce code ligne par ligne : ", "icon": "📖"},{"label": "Optimiser", "prompt": "Comment optimiser ce code ? ", "icon": "⚡"},{"label": "Écrire des tests", "prompt": "Écris des tests unitaires pour : ", "icon": "✅"}]'),
(x'3ef5f41850ad4f639ef6645ab295e557', 'Rédacteur',
 'Expert en rédaction — articles, emails, rapports, reformulation', '✍️', '#1e8e3e',
 'Tu es un expert en rédaction et communication écrite. Tu aides à rédiger, reformuler, améliorer et corriger des textes. Adapte ton style au contexte demandé.',
 '[]', 1,
 '[{"label": "Améliorer ce texte", "prompt": "Améliore ce texte en gardant le sens : ", "icon": "✨"},{"label": "Email professionnel", "prompt": "Rédige un email professionnel pour : ", "icon": "📧"},{"label": "Résumé exécutif", "prompt": "Fais un résumé exécutif de : ", "icon": "📋"},{"label": "Article de blog", "prompt": "Rédige un article de blog sur : ", "icon": "📰"}]');

CREATE TABLE assistant.provider_config (
    provider      TEXT    NOT NULL PRIMARY KEY
                      CHECK (provider IN ('ollama','openai','anthropic','google')),
    enabled       INTEGER NOT NULL DEFAULT 0,
    api_key       TEXT    NOT NULL DEFAULT '',
    base_url      TEXT    NOT NULL DEFAULT '',
    default_model TEXT    NOT NULL DEFAULT '',
    updated_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
);
INSERT INTO assistant.provider_config (provider, enabled, base_url, default_model) VALUES
    ('ollama',    1, 'http://localhost:11434', 'llama3.2:3b'),
    ('openai',    0, 'https://api.openai.com/v1', 'gpt-4o-mini'),
    ('anthropic', 0, 'https://api.anthropic.com', 'claude-3-5-haiku-20241022'),
    ('google',    0, 'https://generativelanguage.googleapis.com', 'gemini-2.0-flash');

CREATE TABLE assistant.folders (
    id         BLOB    NOT NULL PRIMARY KEY,
    owner_id   BLOB    NOT NULL,
    name       TEXT    NOT NULL,
    color      TEXT,
    position   INTEGER NOT NULL DEFAULT 0,
    change_seq INTEGER NOT NULL DEFAULT 0,
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')),
    updated_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
);
CREATE INDEX assistant.idx_assistant_folders_owner       ON folders(owner_id);
CREATE INDEX assistant.idx_assistant_folder_change_seq   ON folders(owner_id, change_seq);

-- ── Delta journal: one shared counter, one tombstone table per entity ─────────

CREATE TABLE assistant.change_counter (
    domain TEXT    NOT NULL PRIMARY KEY,
    n      INTEGER NOT NULL
);

CREATE TABLE assistant.conv_tombstones (
    id         BLOB    NOT NULL PRIMARY KEY,
    owner_id   BLOB    NOT NULL,
    change_seq INTEGER NOT NULL,
    deleted_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
);
CREATE INDEX assistant.idx_assistant_conv_tomb ON conv_tombstones(owner_id, change_seq);

CREATE TABLE assistant.folder_tombstones (
    id         BLOB    NOT NULL PRIMARY KEY,
    owner_id   BLOB    NOT NULL,
    change_seq INTEGER NOT NULL,
    deleted_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
);
CREATE INDEX assistant.idx_assistant_folder_tomb ON folder_tombstones(owner_id, change_seq);

CREATE TABLE assistant.agent_tombstones (
    id         BLOB    NOT NULL PRIMARY KEY,
    owner_id   BLOB,
    change_seq INTEGER NOT NULL,
    deleted_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now'))
);
CREATE INDEX assistant.idx_assistant_agent_tomb ON agent_tombstones(owner_id, change_seq);
