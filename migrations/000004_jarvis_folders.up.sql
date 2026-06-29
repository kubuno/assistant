-- Dossiers pour organiser/regrouper les conversations.
CREATE TABLE jarvis.folders (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id   UUID NOT NULL,
    name       VARCHAR(120) NOT NULL,
    color      VARCHAR(20),
    position   INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_jarvis_folders_owner ON jarvis.folders(owner_id);

-- Rattachement d'une conversation à un dossier (NULL = « Sans dossier »).
ALTER TABLE jarvis.conversations
    ADD COLUMN folder_id UUID REFERENCES jarvis.folders(id) ON DELETE SET NULL;
CREATE INDEX idx_jarvis_conv_folder ON jarvis.conversations(folder_id);
