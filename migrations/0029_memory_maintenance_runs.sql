-- Reuse the run ledger for Memory capture/review checkpoints and recovery.
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
    (kind='slack_interaction' AND status IN ('open','running','completed','failed','reversed')) OR
    (kind IN ('human_mutation','system_mutation','mutation','legacy_import')
        AND status IN ('open','running','completed','failed','reversed'))
);

CREATE INDEX memory_dream_reviewed_idx ON runs USING gin (result jsonb_path_ops)
  WHERE kind='memory_dream' AND status='completed';
