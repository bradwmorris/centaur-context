-- Binary supporting visuals share Artifact identity, but not text columns or indexes.
ALTER TABLE artifacts DROP CONSTRAINT artifacts_check;
ALTER TABLE artifacts ADD CONSTRAINT artifacts_content_or_uri_or_visual_check
    CHECK (content IS NOT NULL OR uri IS NOT NULL OR kind='research_visual');
ALTER TABLE artifacts DROP CONSTRAINT artifacts_complete_content_check;
ALTER TABLE artifacts ADD CONSTRAINT artifacts_complete_content_check
    CHECK (capture_outcome<>'complete' OR content IS NOT NULL OR kind='research_visual');

CREATE TABLE artifact_binary_payloads (
    artifact_id uuid PRIMARY KEY REFERENCES artifacts(id) ON DELETE RESTRICT,
    media_type text NOT NULL CHECK (media_type IN ('image/png','image/jpeg')),
    width integer NOT NULL CHECK (width > 0),
    height integer NOT NULL CHECK (height > 0),
    bytes bytea NOT NULL CHECK (octet_length(bytes) BETWEEN 1 AND 20971520)
);

CREATE TRIGGER artifact_binary_payloads_are_immutable
BEFORE UPDATE OR DELETE ON artifact_binary_payloads
FOR EACH ROW EXECUTE FUNCTION preserve_artifact();

CREATE OR REPLACE FUNCTION require_visual_payload() RETURNS trigger
LANGUAGE plpgsql AS $$ BEGIN
    IF NEW.kind='research_visual' AND NOT EXISTS (
        SELECT 1 FROM artifact_binary_payloads WHERE artifact_id=NEW.id
    ) THEN
        RAISE EXCEPTION 'research_visual requires a binary payload';
    END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER research_visual_has_payload
AFTER INSERT ON artifacts DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION require_visual_payload();

CREATE TABLE visual_upload_requests (
    actor_type text NOT NULL,
    actor_id text NOT NULL,
    idempotency_key text NOT NULL,
    request_hash text NOT NULL,
    artifact_id uuid NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    PRIMARY KEY (actor_type,actor_id,idempotency_key)
);
