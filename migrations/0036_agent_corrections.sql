-- Corrections preserve original evidence and authenticated attribution.
CREATE TABLE evidence_corrections (
    id uuid PRIMARY KEY,
    object_id uuid NOT NULL REFERENCES objects(id) ON DELETE RESTRICT,
    target_type text NOT NULL CHECK (target_type IN ('object','artifact','message','event','run')),
    target_id uuid NOT NULL,
    reason text NOT NULL CHECK (char_length(btrim(reason)) BETWEEN 1 AND 2000),
    representation jsonb NOT NULL CHECK (jsonb_typeof(representation)='object' AND pg_column_size(representation)<=262144),
    supersedes_correction_id uuid REFERENCES evidence_corrections(id) ON DELETE RESTRICT,
    actor_type text NOT NULL,
    actor_id text NOT NULL,
    object_revision bigint NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(object_id,object_revision)
);
CREATE INDEX evidence_corrections_target_idx ON evidence_corrections(object_id,target_type,target_id,object_revision DESC);
CREATE TRIGGER evidence_corrections_immutable BEFORE UPDATE OR DELETE ON evidence_corrections
FOR EACH ROW EXECUTE FUNCTION preserve_artifact();

-- Explicit edits own current display fields; automated capture still appends evidence.
ALTER TABLE objects ADD COLUMN explicitly_corrected boolean NOT NULL DEFAULT false;
COMMENT ON COLUMN objects.protected IS 'Preference against autonomous curation; does not block explicitly authorized revisioned corrections.';
COMMENT ON COLUMN connections.protected IS 'Preference against autonomous curation; does not block explicitly authorized revisioned corrections.';
