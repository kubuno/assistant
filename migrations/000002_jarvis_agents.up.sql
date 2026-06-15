-- =====================
-- AGENTS PERSONNALISÉS
-- =====================
CREATE TABLE jarvis.agents (
    id                 UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id           UUID,
    name               VARCHAR(255) NOT NULL,
    description        TEXT,
    avatar_emoji       VARCHAR(10)  NOT NULL DEFAULT '🤖',
    avatar_color       VARCHAR(7)   NOT NULL DEFAULT '#1a73e8',
    system_prompt      TEXT NOT NULL DEFAULT '',
    preferred_model    VARCHAR(100),
    preferred_provider VARCHAR(20),
    generation_params  JSONB NOT NULL DEFAULT '{}',
    enabled_tools      TEXT[] NOT NULL DEFAULT '{}',
    prompt_suggestions JSONB NOT NULL DEFAULT '[]',
    is_public          BOOLEAN NOT NULL DEFAULT FALSE,
    is_system          BOOLEAN NOT NULL DEFAULT FALSE,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_jarvis_agents_owner  ON jarvis.agents(owner_id);
CREATE INDEX idx_jarvis_agents_public ON jarvis.agents(is_public) WHERE is_public = TRUE;
CREATE INDEX idx_jarvis_agents_system ON jarvis.agents(is_system) WHERE is_system = TRUE;

-- Agents système prédéfinis
INSERT INTO jarvis.agents
    (name, description, avatar_emoji, avatar_color, system_prompt,
     enabled_tools, is_system, prompt_suggestions)
VALUES
(
    'Jarvis',
    'Assistant général Kubuno — accès à tous vos modules',
    '🤖', '#1a73e8',
    'Tu es Jarvis, l''assistant IA de Kubuno, une plateforme cloud self-hosted. '
    'Tu es toujours utile, concis et précis. '
    'Réponds en français sauf si l''utilisateur écrit dans une autre langue.',
    ARRAY[]::text[],
    TRUE,
    '[
        {"label": "Résume mes notes récentes", "prompt": "Résume les notes que j''ai créées cette semaine", "icon": "📝"},
        {"label": "Explique-moi…", "prompt": "Explique-moi en détail : ", "icon": "💡"},
        {"label": "Rédiger un email", "prompt": "Aide-moi à rédiger un email professionnel sur : ", "icon": "✉️"},
        {"label": "Analyser du code", "prompt": "Analyse et explique ce code : ", "icon": "💻"}
    ]'::jsonb
),
(
    'Expert Code',
    'Développeur senior — explique, débogue et génère du code dans tous les langages',
    '💻', '#e8824a',
    'Tu es un expert en développement logiciel. '
    'Tu aides à écrire, déboguer et expliquer du code dans tous les langages. '
    'Préfère les solutions simples et bien documentées. '
    'Utilise toujours des blocs de code avec la syntaxe appropriée.',
    ARRAY[]::text[],
    TRUE,
    '[
        {"label": "Déboguer ce code", "prompt": "Voici du code qui ne fonctionne pas : ", "icon": "🐛"},
        {"label": "Expliquer ligne par ligne", "prompt": "Explique ce code ligne par ligne : ", "icon": "📖"},
        {"label": "Optimiser", "prompt": "Comment optimiser ce code ? ", "icon": "⚡"},
        {"label": "Écrire des tests", "prompt": "Écris des tests unitaires pour : ", "icon": "✅"}
    ]'::jsonb
),
(
    'Rédacteur',
    'Expert en rédaction — articles, emails, rapports, reformulation',
    '✍️', '#1e8e3e',
    'Tu es un expert en rédaction et communication écrite. '
    'Tu aides à rédiger, reformuler, améliorer et corriger des textes. '
    'Adapte ton style au contexte demandé.',
    ARRAY[]::text[],
    TRUE,
    '[
        {"label": "Améliorer ce texte", "prompt": "Améliore ce texte en gardant le sens : ", "icon": "✨"},
        {"label": "Email professionnel", "prompt": "Rédige un email professionnel pour : ", "icon": "📧"},
        {"label": "Résumé exécutif", "prompt": "Fais un résumé exécutif de : ", "icon": "📋"},
        {"label": "Article de blog", "prompt": "Rédige un article de blog sur : ", "icon": "📰"}
    ]'::jsonb
);
