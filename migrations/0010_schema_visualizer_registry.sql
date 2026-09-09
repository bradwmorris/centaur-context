CREATE TABLE schema_visualizer_tables (
    table_name text PRIMARY KEY CHECK (table_name ~ '^[a-z][a-z0-9_]*$'),
    registered_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO schema_visualizer_tables (table_name) VALUES
    ('objects'),
    ('connections'),
    ('tasks'),
    ('object_events'),
    ('chats'),
    ('entities'),
    ('memories'),
    ('users'),
    ('external_identities'),
    ('chat_messages'),
    ('curator_runs'),
    ('object_embeddings'),
    ('object_embedding_jobs'),
    ('curator_run_changes'),
    ('evals'),
    ('eval_trace_entries'),
    ('eval_objects');

COMMENT ON TABLE schema_visualizer_tables IS
    'Migration-owned allowlist for the trusted human read-only schema visualizer.';
