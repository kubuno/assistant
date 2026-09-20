-- MySQL / MariaDB — the `assistant` database is created by kubuno-db's
-- `ensure_schema` before the migrator runs, so there is no CREATE DATABASE here.
-- This single file declares the FINAL shape the PostgreSQL side reached across
-- its 000001..000008 migrations (delta journal included, `enabled_tools` as
-- JSON), stated once.
--
-- Differences from PostgreSQL, and why:
--   * UUID -> BINARY(16): what sqlx encodes a `uuid::Uuid` as on MySQL.
--   * No DEFAULT on `id`: MySQL has no gen_random_uuid() and no RETURNING, so
--     the process supplies every primary key.
--   * TIMESTAMPTZ -> DATETIME(6); every value written is UTC (the pool pins
--     `time_zone = '+00:00'`).
--   * JSONB / TEXT[] -> JSON (generation_params, tool_calls, enabled_tools, …).
--   * The conversations `updated_at` is maintained by ON UPDATE CURRENT_TIMESTAMP(6)
--     (the other tables' `updated_at` are set by the application, matching the
--     PostgreSQL side, so they carry no ON UPDATE clause).
--   * `message_count` / `total_tokens` are maintained by the application, not a
--     trigger; the delta layer is the journal (change_counter + change_seq).
--   * No partial indexes (MySQL has none): the WHERE-filtered indexes become
--     plain indexes.

CREATE TABLE assistant.conversations (
    id                BINARY(16)   NOT NULL PRIMARY KEY,
    owner_id          BINARY(16)   NOT NULL,
    title             VARCHAR(500) NULL,
    agent_id          BINARY(16)   NULL,
    model_id          VARCHAR(100) NOT NULL DEFAULT 'llama3.2:3b',
    provider          VARCHAR(20)  NOT NULL DEFAULT 'ollama'
                          CHECK (provider IN ('ollama','openai','anthropic','google')),
    memory_summary    TEXT         NULL,
    generation_params JSON         NOT NULL DEFAULT (JSON_OBJECT('temperature', 0.7, 'top_p', 0.9, 'max_tokens', 4096)),
    is_pinned         BOOLEAN      NOT NULL DEFAULT FALSE,
    is_archived       BOOLEAN      NOT NULL DEFAULT FALSE,
    is_trashed        BOOLEAN      NOT NULL DEFAULT FALSE,
    message_count     INT          NOT NULL DEFAULT 0,
    total_tokens      INT          NOT NULL DEFAULT 0,
    folder_id         BINARY(16)   NULL,
    position          INT          NOT NULL DEFAULT 0,
    change_seq        BIGINT       NOT NULL DEFAULT 0,
    created_at        DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at        DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
                                   ON UPDATE CURRENT_TIMESTAMP(6)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_assistant_conv_owner      ON assistant.conversations(owner_id, updated_at);
CREATE INDEX idx_assistant_conv_pinned     ON assistant.conversations(owner_id, is_pinned);
CREATE INDEX idx_assistant_conv_folder     ON assistant.conversations(folder_id);
CREATE INDEX idx_assistant_conv_position   ON assistant.conversations(owner_id, position, updated_at);
CREATE INDEX idx_assistant_conv_change_seq ON assistant.conversations(owner_id, change_seq);

CREATE TABLE assistant.messages (
    id                BINARY(16)  NOT NULL PRIMARY KEY,
    conversation_id   BINARY(16)  NOT NULL,
    role              VARCHAR(15) NOT NULL CHECK (role IN ('user', 'assistant', 'system', 'tool')),
    content           TEXT        NOT NULL,
    attachments       JSON        NOT NULL DEFAULT (JSON_ARRAY()),
    tool_calls        JSON        NOT NULL DEFAULT (JSON_ARRAY()),
    rag_sources       JSON        NOT NULL DEFAULT (JSON_ARRAY()),
    prompt_tokens     INT         NOT NULL DEFAULT 0,
    completion_tokens INT         NOT NULL DEFAULT 0,
    generation_ms     INT         NULL,
    feedback          VARCHAR(10) NULL CHECK (feedback IN ('like', 'dislike')),
    is_regenerated    BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at        DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    FOREIGN KEY (conversation_id) REFERENCES assistant.conversations(id) ON DELETE CASCADE
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_assistant_messages_conv ON assistant.messages(conversation_id, created_at);

CREATE TABLE assistant.agents (
    id                 BINARY(16)   NOT NULL PRIMARY KEY,
    owner_id           BINARY(16)   NULL,
    name               VARCHAR(255) NOT NULL,
    description        TEXT         NULL,
    avatar_emoji       VARCHAR(10)  NOT NULL DEFAULT '🤖',
    avatar_color       VARCHAR(7)   NOT NULL DEFAULT '#1a73e8',
    system_prompt      TEXT         NOT NULL,
    preferred_model    VARCHAR(100) NULL,
    preferred_provider VARCHAR(20)  NULL,
    generation_params  JSON         NOT NULL DEFAULT (JSON_OBJECT()),
    enabled_tools      JSON         NOT NULL DEFAULT (JSON_ARRAY()),
    prompt_suggestions JSON         NOT NULL DEFAULT (JSON_ARRAY()),
    is_public          BOOLEAN      NOT NULL DEFAULT FALSE,
    is_system          BOOLEAN      NOT NULL DEFAULT FALSE,
    change_seq         BIGINT       NOT NULL DEFAULT 0,
    created_at         DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at         DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_assistant_agents_owner      ON assistant.agents(owner_id);
CREATE INDEX idx_assistant_agents_public     ON assistant.agents(is_public);
CREATE INDEX idx_assistant_agents_system     ON assistant.agents(is_system);
CREATE INDEX idx_assistant_agent_change_seq  ON assistant.agents(change_seq);

INSERT INTO assistant.agents (id, name, description, avatar_emoji, avatar_color, system_prompt, generation_params, enabled_tools, prompt_suggestions, is_system) VALUES
(UNHEX('493a9330064e4ec9b1525dadfece6f2c'), 'Assistant',
 'Assistant général Kubuno — accès à tous vos modules', '🤖', '#1a73e8',
 'Tu es Assistant, l''assistant IA de Kubuno, une plateforme cloud self-hosted. Tu es toujours utile, concis et précis. Réponds en français sauf si l''utilisateur écrit dans une autre langue.',
 '{}', '[]',
 '[{"label": "Résume mes notes récentes", "prompt": "Résume les notes que j''ai créées cette semaine", "icon": "📝"},{"label": "Explique-moi…", "prompt": "Explique-moi en détail : ", "icon": "💡"},{"label": "Rédiger un email", "prompt": "Aide-moi à rédiger un email professionnel sur : ", "icon": "✉️"},{"label": "Analyser du code", "prompt": "Analyse et explique ce code : ", "icon": "💻"}]', TRUE),
(UNHEX('9d2037dd899643fdaf240e41748aa666'), 'Expert Code',
 'Développeur senior — explique, débogue et génère du code dans tous les langages', '💻', '#e8824a',
 'Tu es un expert en développement logiciel. Tu aides à écrire, déboguer et expliquer du code dans tous les langages. Préfère les solutions simples et bien documentées. Utilise toujours des blocs de code avec la syntaxe appropriée.',
 '{}', '[]',
 '[{"label": "Déboguer ce code", "prompt": "Voici du code qui ne fonctionne pas : ", "icon": "🐛"},{"label": "Expliquer ligne par ligne", "prompt": "Explique ce code ligne par ligne : ", "icon": "📖"},{"label": "Optimiser", "prompt": "Comment optimiser ce code ? ", "icon": "⚡"},{"label": "Écrire des tests", "prompt": "Écris des tests unitaires pour : ", "icon": "✅"}]', TRUE),
(UNHEX('3ef5f41850ad4f639ef6645ab295e557'), 'Rédacteur',
 'Expert en rédaction — articles, emails, rapports, reformulation', '✍️', '#1e8e3e',
 'Tu es un expert en rédaction et communication écrite. Tu aides à rédiger, reformuler, améliorer et corriger des textes. Adapte ton style au contexte demandé.',
 '{}', '[]',
 '[{"label": "Améliorer ce texte", "prompt": "Améliore ce texte en gardant le sens : ", "icon": "✨"},{"label": "Email professionnel", "prompt": "Rédige un email professionnel pour : ", "icon": "📧"},{"label": "Résumé exécutif", "prompt": "Fais un résumé exécutif de : ", "icon": "📋"},{"label": "Article de blog", "prompt": "Rédige un article de blog sur : ", "icon": "📰"}]', TRUE);

CREATE TABLE assistant.provider_config (
    provider      VARCHAR(20)  NOT NULL PRIMARY KEY
                      CHECK (provider IN ('ollama','openai','anthropic','google')),
    enabled       BOOLEAN      NOT NULL DEFAULT FALSE,
    api_key       TEXT         NOT NULL,
    base_url      TEXT         NOT NULL,
    default_model VARCHAR(100) NOT NULL DEFAULT '',
    updated_at    DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
INSERT INTO assistant.provider_config (provider, enabled, api_key, base_url, default_model) VALUES
    ('ollama',    TRUE,  '', 'http://localhost:11434', 'llama3.2:3b'),
    ('openai',    FALSE, '', 'https://api.openai.com/v1', 'gpt-4o-mini'),
    ('anthropic', FALSE, '', 'https://api.anthropic.com', 'claude-3-5-haiku-20241022'),
    ('google',    FALSE, '', 'https://generativelanguage.googleapis.com', 'gemini-2.0-flash');

CREATE TABLE assistant.folders (
    id         BINARY(16)   NOT NULL PRIMARY KEY,
    owner_id   BINARY(16)   NOT NULL,
    name       VARCHAR(120) NOT NULL,
    color      VARCHAR(20)  NULL,
    position   INT          NOT NULL DEFAULT 0,
    change_seq BIGINT       NOT NULL DEFAULT 0,
    created_at DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_assistant_folders_owner     ON assistant.folders(owner_id);
CREATE INDEX idx_assistant_folder_change_seq ON assistant.folders(owner_id, change_seq);

-- The conversations -> folders link is created after folders exists.
ALTER TABLE assistant.conversations
    ADD FOREIGN KEY (folder_id) REFERENCES assistant.folders(id) ON DELETE SET NULL;

-- ── Delta journal: one shared counter, one tombstone table per entity ─────────

CREATE TABLE assistant.change_counter (
    domain VARCHAR(190) NOT NULL PRIMARY KEY,
    n      BIGINT       NOT NULL
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE assistant.conv_tombstones (
    id         BINARY(16)  NOT NULL PRIMARY KEY,
    owner_id   BINARY(16)  NOT NULL,
    change_seq BIGINT      NOT NULL,
    deleted_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_assistant_conv_tomb ON assistant.conv_tombstones(owner_id, change_seq);

CREATE TABLE assistant.folder_tombstones (
    id         BINARY(16)  NOT NULL PRIMARY KEY,
    owner_id   BINARY(16)  NOT NULL,
    change_seq BIGINT      NOT NULL,
    deleted_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_assistant_folder_tomb ON assistant.folder_tombstones(owner_id, change_seq);

CREATE TABLE assistant.agent_tombstones (
    id         BINARY(16)  NOT NULL PRIMARY KEY,
    owner_id   BINARY(16)  NULL,
    change_seq BIGINT      NOT NULL,
    deleted_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
CREATE INDEX idx_assistant_agent_tomb ON assistant.agent_tombstones(owner_id, change_seq);
