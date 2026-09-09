-- Stable recently-created Object lists, both across all kinds and within a kind.
CREATE INDEX objects_active_created_idx
    ON objects (created_at DESC, id DESC)
    WHERE archived_at IS NULL;

CREATE INDEX objects_active_kind_created_idx
    ON objects (kind, created_at DESC, id DESC)
    WHERE archived_at IS NULL;

-- Single-column partial endpoint indexes keep both halves of density counts
-- bounded without scanning archived Connection history or composite keys.
CREATE INDEX connections_active_source_idx
    ON connections (source_object_id)
    WHERE archived_at IS NULL;

CREATE INDEX connections_active_target_idx
    ON connections (target_object_id)
    WHERE archived_at IS NULL;
