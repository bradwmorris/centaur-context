-- Codex is a distinct provider. Existing Slack identities and history are preserved.
ALTER TABLE chats DROP CONSTRAINT chats_provider_identity_check;
ALTER TABLE chats ADD CONSTRAINT chats_provider_identity_check CHECK (
    (provider IS NULL AND workspace_id IS NULL AND channel_id IS NULL
        AND thread_id IS NULL AND surface_kind IS NULL)
    OR
    (char_length(btrim(provider)) BETWEEN 1 AND 100
        AND char_length(btrim(workspace_id)) BETWEEN 1 AND 300
        AND char_length(btrim(channel_id)) BETWEEN 1 AND 300
        AND char_length(btrim(thread_id)) BETWEEN 1 AND 300
        AND (surface_kind IN ('channel','dm') OR (provider='codex' AND surface_kind='desktop')))
);
CREATE UNIQUE INDEX chats_codex_host_session_idx ON chats(workspace_id,thread_id) WHERE provider='codex';

ALTER TABLE runs DROP CONSTRAINT runs_kind_status_check;
ALTER TABLE runs ADD CONSTRAINT runs_kind_status_check CHECK (
    (kind='external_action' AND status IN (
        'reserved','previewed','approved','attempting','reconciliation_required',
        'accepted','delivered','suppressed','failed'
    )) OR
    (kind IN ('memory_capture_start','memory_capture','memory_dream','memory_undo') AND status IN ('running','completed','preview','failed','reversed')) OR
    (kind='curator' AND status IN ('queued','running','completed','failed','reversed')) OR
    (kind='curator_undo' AND status IN ('running','completed','failed')) OR
    (kind='intake' AND status IN ('running','completed','failed')) OR
    (kind='workflow' AND status IN ('running','completed','failed')) OR
    (kind IN ('slack_interaction','codex_interaction') AND status IN ('open','running','completed','failed','reversed')) OR
    (kind IN ('human_mutation','system_mutation','mutation','legacy_import')
        AND status IN ('open','running','completed','failed','reversed'))
);
