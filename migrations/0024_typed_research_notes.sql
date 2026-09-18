ALTER TABLE notes
    ADD COLUMN intent text,
    ADD COLUMN source_artifact_id uuid REFERENCES artifacts(id) ON DELETE RESTRICT,
    ADD COLUMN source_locator jsonb;

ALTER TABLE notes
    ADD CONSTRAINT notes_intent_check
    CHECK (intent IS NULL OR intent IN ('excerpt','insight','question')),
    ADD CONSTRAINT notes_source_evidence_check
    CHECK (
        (intent = 'excerpt' AND source_artifact_id IS NOT NULL AND source_locator IS NOT NULL)
        OR
        (intent IS DISTINCT FROM 'excerpt' AND source_artifact_id IS NULL AND source_locator IS NULL)
    ),
    ADD CONSTRAINT notes_source_locator_object_check
    CHECK (source_locator IS NULL OR jsonb_typeof(source_locator) = 'object');

CREATE INDEX notes_intent_object_idx ON notes(intent,object_id) WHERE intent IS NOT NULL;

COMMENT ON COLUMN notes.intent IS
    'Durable atomic research intent: excerpt, insight, or question. NULL is reserved for legacy Notes.';
COMMENT ON COLUMN notes.source_artifact_id IS
    'Evidence Artifact for an excerpt Note; must belong to its single derived Source.';
COMMENT ON COLUMN notes.source_locator IS
    'Validated heterogeneous locator for an excerpt: timestamp, page, section, or text_offset.';
