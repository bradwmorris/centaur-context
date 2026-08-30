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
