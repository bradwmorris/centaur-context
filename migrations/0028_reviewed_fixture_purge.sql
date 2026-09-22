-- Payload-free, durable receipts for separately owner-approved fixture deletion.
CREATE TABLE maintenance_purge_receipts (
    principal_id text NOT NULL,
    idempotency_key text NOT NULL,
    request_sha256 text NOT NULL,
    manifest_sha256 text NOT NULL,
    receipt jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (principal_id,idempotency_key)
);

-- An exact transaction-local allowlist is only installed by reviewed maintenance.
-- Ordinary UPDATEs and DELETEs keep their immutable protection.
CREATE OR REPLACE FUNCTION preserve_artifact() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE permitted boolean := false;
BEGIN
    IF TG_OP='DELETE' AND to_regclass('pg_temp.context_reviewed_purge') IS NOT NULL THEN
        EXECUTE 'SELECT EXISTS (SELECT 1 FROM pg_temp.context_reviewed_purge WHERE table_name=$1 AND row_id=$2)'
            INTO permitted USING TG_TABLE_NAME,OLD.id;
        IF permitted THEN RETURN OLD; END IF;
    END IF;
    RAISE EXCEPTION 'Artifacts are immutable';
END $$;
CREATE OR REPLACE FUNCTION preserve_object_event() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE permitted boolean := false;
BEGIN
    IF TG_OP='DELETE' AND to_regclass('pg_temp.context_reviewed_purge') IS NOT NULL THEN
        EXECUTE 'SELECT EXISTS (SELECT 1 FROM pg_temp.context_reviewed_purge WHERE table_name=$1 AND row_id=$2)'
            INTO permitted USING TG_TABLE_NAME,OLD.id;
        IF permitted THEN RETURN OLD; END IF;
    END IF;
    RAISE EXCEPTION 'Object Events are immutable';
END $$;
