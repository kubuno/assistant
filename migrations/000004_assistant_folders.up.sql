-- Dossiers pour organiser/regrouper les conversations.
CREATE TABLE assistant.folders (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id   UUID NOT NULL,
    name       VARCHAR(120) NOT NULL,
    color      VARCHAR(20),
    position   INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_assistant_folders_owner ON assistant.folders(owner_id);

-- Rattachement d'une conversation à un dossier (NULL = « Sans dossier »).
ALTER TABLE assistant.conversations
    ADD COLUMN folder_id UUID REFERENCES assistant.folders(id) ON DELETE SET NULL;
CREATE INDEX idx_assistant_conv_folder ON assistant.conversations(folder_id);
