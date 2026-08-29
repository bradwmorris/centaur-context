ALTER TABLE objects DROP CONSTRAINT objects_kind_check;
ALTER TABLE objects ADD CONSTRAINT objects_kind_check
    CHECK (kind IN ('note', 'source', 'decision', 'task', 'chat', 'entity', 'memory'));

CREATE TABLE chats (
    object_id uuid PRIMARY KEY,
    object_kind text NOT NULL DEFAULT 'chat' CHECK (object_kind = 'chat'),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (object_id, object_kind) REFERENCES objects(id, kind) ON DELETE RESTRICT
);

CREATE TABLE entities (
    object_id uuid PRIMARY KEY,
    object_kind text NOT NULL DEFAULT 'entity' CHECK (object_kind = 'entity'),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (object_id, object_kind) REFERENCES objects(id, kind) ON DELETE RESTRICT
);

CREATE TABLE memories (
    object_id uuid PRIMARY KEY,
    object_kind text NOT NULL DEFAULT 'memory' CHECK (object_kind = 'memory'),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (object_id, object_kind) REFERENCES objects(id, kind) ON DELETE RESTRICT
);

CREATE TABLE notes (
    object_id uuid PRIMARY KEY,
    object_kind text NOT NULL DEFAULT 'note' CHECK (object_kind = 'note'),
    content text NOT NULL CHECK (char_length(btrim(content)) BETWEEN 1 AND 100000),
    content_format text NOT NULL DEFAULT 'markdown' CHECK (content_format IN ('plain_text', 'markdown')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (object_id, object_kind) REFERENCES objects(id, kind) ON DELETE RESTRICT
);

CREATE INDEX notes_search_idx ON notes USING gin (to_tsvector('simple', content));

CREATE TABLE sources (
    object_id uuid PRIMARY KEY,
    object_kind text NOT NULL DEFAULT 'source' CHECK (object_kind = 'source'),
    source_kind text NOT NULL CHECK (source_kind IN (
        'article', 'paper', 'podcast', 'video', 'book', 'report',
        'document', 'dataset', 'web_page', 'other'
    )),
    canonical_uri text CHECK (
        canonical_uri IS NULL OR
        (char_length(canonical_uri) <= 2000 AND canonical_uri ~* '^https?://')
    ),
    byline text CHECK (byline IS NULL OR char_length(btrim(byline)) BETWEEN 1 AND 500),
    publisher text CHECK (publisher IS NULL OR char_length(btrim(publisher)) BETWEEN 1 AND 300),
    published_at timestamptz,
    accessed_at timestamptz,
    language text CHECK (language IS NULL OR char_length(btrim(language)) BETWEEN 1 AND 35),
    media_type text CHECK (media_type IS NULL OR char_length(btrim(media_type)) BETWEEN 1 AND 255),
    artifact_reference text CHECK (artifact_reference IS NULL OR char_length(btrim(artifact_reference)) BETWEEN 1 AND 1000),
    content_hash text CHECK (content_hash IS NULL OR content_hash ~ '^[0-9a-f]{64}$'),
    current_content_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (object_id, object_kind) REFERENCES objects(id, kind) ON DELETE RESTRICT
);

CREATE INDEX sources_list_idx ON sources (source_kind, published_at DESC NULLS LAST, object_id);

CREATE TABLE source_contents (
    id uuid PRIMARY KEY,
    source_object_id uuid NOT NULL REFERENCES sources(object_id) ON DELETE RESTRICT,
    version bigint NOT NULL CHECK (version > 0),
    content_kind text NOT NULL CHECK (content_kind IN ('article_text', 'transcript', 'paper_text', 'document_text', 'dataset_description', 'other')),
    normalized_text text NOT NULL CHECK (char_length(normalized_text) > 0),
    language text CHECK (language IS NULL OR char_length(btrim(language)) BETWEEN 1 AND 35),
    extraction_method text CHECK (extraction_method IS NULL OR char_length(btrim(extraction_method)) BETWEEN 1 AND 200),
    extraction_version text CHECK (extraction_version IS NULL OR char_length(btrim(extraction_version)) BETWEEN 1 AND 100),
    content_hash text NOT NULL CHECK (content_hash ~ '^[0-9a-f]{64}$'),
    size_bytes bigint NOT NULL CHECK (size_bytes > 0 AND size_bytes = octet_length(normalized_text)),
    artifact_reference text CHECK (artifact_reference IS NULL OR char_length(btrim(artifact_reference)) BETWEEN 1 AND 1000),
    locators jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(locators) = 'object'),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (source_object_id, version),
    UNIQUE (source_object_id, id)
);

ALTER TABLE sources
    ADD CONSTRAINT sources_current_content_fk
    FOREIGN KEY (object_id, current_content_id)
    REFERENCES source_contents(source_object_id, id) ON DELETE RESTRICT;

CREATE INDEX source_contents_source_idx ON source_contents (source_object_id, version DESC);
CREATE INDEX source_contents_search_idx ON source_contents USING gin (to_tsvector('simple', normalized_text));

CREATE OR REPLACE FUNCTION preserve_source_content()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'Source content versions are immutable';
END
$$;

CREATE TRIGGER source_contents_are_immutable
BEFORE UPDATE OR DELETE ON source_contents
FOR EACH ROW EXECUTE FUNCTION preserve_source_content();
