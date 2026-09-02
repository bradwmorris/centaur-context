#!/usr/bin/env python3
"""Preview or delete one exact local RSI evaluation fixture.

The command is deliberately Kubernetes/local-lab specific. It never discovers a
database by URL, requires explicit Object IDs, prints a hashable manifest, and
requires that exact hash plus an approval phrase before executing.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from typing import Any
from uuid import UUID


EXPECTED_CONTEXT = "kind-centaur-lab"
EXPECTED_DATABASE = "centaur_context_enyu"
EXPECTED_SOURCE_URI = "https://www.youtube.com/watch?v=hY6S__xeCjg"
APPROVAL = "DELETE_EXACT_RSI_EVAL_FIXTURE"


def uuid(value: str) -> str:
    return str(UUID(value))


def run(*args: str) -> str:
    completed = subprocess.run(args, text=True, capture_output=True)
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RuntimeError(f"command failed ({completed.returncode}): {detail}")
    return completed.stdout.strip()


def psql(namespace: str, pod: str, sql: str) -> str:
    return run(
        "kubectl", "-n", namespace, "exec", pod, "--", "psql", "-X", "-U", "tempo",
        "-d", EXPECTED_DATABASE, "-v", "ON_ERROR_STOP=1", "-At", "-F", "\t", "-c", sql,
    )


def rows(namespace: str, pod: str, sql: str) -> list[list[str]]:
    output = psql(namespace, pod, sql)
    return [line.split("\t") for line in output.splitlines() if line]


def quoted_ids(values: list[str]) -> str:
    return ",".join(f"'{value}'::uuid" for value in values)


def manifest(namespace: str, pod: str, object_ids: list[str]) -> dict[str, Any]:
    explicit_ids = list(object_ids)
    source_id = object_ids[0]
    selected_objects = set(object_ids)
    selected_connections: set[str] = set()
    selected_runs: set[str] = set()
    for _ in range(20):
        ids = quoted_ids(sorted(selected_objects))
        connection_rows = rows(namespace, pod, f"""
            SELECT id FROM connections
            WHERE source_object_id IN ({ids}) OR target_object_id IN ({ids})
        """)
        selected_connections.update(row[0] for row in connection_rows)
        connection_sql = quoted_ids(sorted(selected_connections)) if selected_connections else "NULL::uuid"
        run_rows = rows(namespace, pod, f"""
            WITH RECURSIVE seed AS (
              SELECT DISTINCT r.id FROM runs r LEFT JOIN object_events e ON e.run_id=r.id
              WHERE r.primary_object_id IN ({ids})
                 OR r.chat_object_id IN ({ids})
                 OR (e.target_type='object' AND e.target_id IN ({ids}))
                 OR (e.target_type='connection' AND e.target_id IN ({connection_sql}))
            ), selected(id) AS (
              SELECT id FROM seed UNION SELECT r.id FROM runs r JOIN selected s ON r.parent_run_id=s.id
            ) SELECT id FROM selected
        """)
        selected_runs.update(row[0] for row in run_rows)
        run_sql = quoted_ids(sorted(selected_runs)) if selected_runs else "NULL::uuid"
        event_objects = rows(namespace, pod, f"""
            SELECT DISTINCT target_id FROM object_events
            WHERE run_id IN ({run_sql}) AND target_type='object'
        """)
        before = (len(selected_objects), len(selected_connections), len(selected_runs))
        selected_objects.update(row[0] for row in event_objects)
        if before == (len(selected_objects), len(selected_connections), len(selected_runs)):
            break
    else:
        raise RuntimeError("dependency closure did not converge")
    object_ids = sorted(selected_objects)
    ids = quoted_ids(object_ids)
    source = rows(namespace, pod, f"""
        SELECT o.id,o.kind,o.title,s.canonical_uri,o.created_at
        FROM objects o JOIN sources s ON s.object_id=o.id
        WHERE o.id='{source_id}'::uuid
    """)
    if len(source) != 1 or source[0][3] != EXPECTED_SOURCE_URI:
        raise RuntimeError("source ID does not resolve to the exact RSI evaluation URL")
    objects = rows(namespace, pod, f"""
        SELECT id,kind,title,created_at FROM objects
        WHERE id IN ({ids}) ORDER BY created_at,id
    """)
    if not set(explicit_ids).issubset({row[0] for row in objects}):
        raise RuntimeError("one or more explicitly requested related Object IDs do not exist")
    connections = rows(namespace, pod, f"""
        SELECT id,kind,source_object_id,target_object_id,created_at
        FROM connections
        WHERE source_object_id IN ({ids}) OR target_object_id IN ({ids})
        ORDER BY created_at,id
    """)
    connection_ids = [row[0] for row in connections]
    connection_sql = quoted_ids(connection_ids) if connection_ids else "NULL::uuid"
    selected_run_sql = quoted_ids(sorted(selected_runs)) if selected_runs else "NULL::uuid"
    runs = rows(namespace, pod, f"""
        SELECT r.id,r.kind,r.status,r.actor_id,COALESCE(r.primary_object_id::text,''),r.created_at
        FROM runs r WHERE r.id IN ({selected_run_sql}) ORDER BY r.created_at,r.id
    """)
    run_ids = [row[0] for row in runs]
    run_sql = quoted_ids(run_ids) if run_ids else "NULL::uuid"
    counts = rows(namespace, pod, f"""
        SELECT
          (SELECT count(*) FROM artifacts WHERE object_id IN ({ids})),
          (SELECT count(*) FROM embeddings WHERE object_id IN ({ids})),
          (SELECT count(*) FROM object_events WHERE run_id IN ({run_sql})),
          (SELECT count(*) FROM object_events WHERE
             (target_type='object' AND target_id IN ({ids})) OR
             (target_type='connection' AND target_id IN ({connection_sql})))
    """)[0]
    return {
        "database": EXPECTED_DATABASE,
        "source": dict(zip(("id", "kind", "title", "canonical_uri", "created_at"), source[0])),
        "objects": [dict(zip(("id", "kind", "title", "created_at"), row)) for row in objects],
        "connections": [dict(zip(("id", "kind", "source_id", "target_id", "created_at"), row)) for row in connections],
        "runs": [dict(zip(("id", "kind", "status", "actor_id", "primary_object_id", "created_at"), row)) for row in runs],
        "dependent_counts": {
            "artifacts": int(counts[0]), "embeddings": int(counts[1]),
            "events_owned_by_runs": int(counts[2]), "events_targeting_records": int(counts[3]),
        },
    }


def digest(value: dict[str, Any]) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(payload).hexdigest()


def delete(namespace: str, pod: str, value: dict[str, Any]) -> None:
    object_ids = [item["id"] for item in value["objects"]]
    connection_ids = [item["id"] for item in value["connections"]]
    run_ids = [item["id"] for item in value["runs"]]
    ids = quoted_ids(object_ids)
    connections = quoted_ids(connection_ids) if connection_ids else "NULL::uuid"
    runs = quoted_ids(run_ids) if run_ids else "NULL::uuid"
    psql(namespace, pod, f"""
        BEGIN;
        SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='30s';
        LOCK TABLE objects,connections,sources,notes,tasks,chats,chat_messages,users,entities,memories,themes,artifacts,embeddings,runs,object_events IN SHARE ROW EXCLUSIVE MODE;
        ALTER TABLE artifacts DISABLE TRIGGER artifacts_are_immutable;
        ALTER TABLE object_events DISABLE TRIGGER object_events_are_immutable;
        DELETE FROM object_events WHERE run_id IN ({runs})
          OR (target_type='object' AND target_id IN ({ids}))
          OR (target_type='connection' AND target_id IN ({connections}));
        DELETE FROM connections WHERE id IN ({connections});
        DELETE FROM runs WHERE id IN ({runs});
        DELETE FROM embeddings WHERE object_id IN ({ids});
        UPDATE chats SET curated_through_message_id=NULL,
          curation_queued_through_message_id=NULL WHERE object_id IN ({ids});
        DELETE FROM chat_messages WHERE chat_object_id IN ({ids});
        DELETE FROM sources WHERE object_id IN ({ids});
        DELETE FROM notes WHERE object_id IN ({ids});
        DELETE FROM tasks WHERE object_id IN ({ids});
        DELETE FROM memories WHERE object_id IN ({ids});
        DELETE FROM themes WHERE object_id IN ({ids});
        DELETE FROM entities WHERE object_id IN ({ids});
        DELETE FROM chats WHERE object_id IN ({ids});
        DELETE FROM users WHERE object_id IN ({ids});
        DELETE FROM artifacts WHERE object_id IN ({ids});
        DELETE FROM objects WHERE id IN ({ids});
        ALTER TABLE artifacts ENABLE TRIGGER artifacts_are_immutable;
        ALTER TABLE object_events ENABLE TRIGGER object_events_are_immutable;
        COMMIT;
    """)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_id", type=uuid)
    parser.add_argument("--related-object-id", action="append", default=[], type=uuid)
    parser.add_argument("--namespace", default="centaur")
    parser.add_argument("--postgres-pod", default="centaur-centaur-postgres-0")
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--manifest-sha256")
    parser.add_argument("--approval")
    args = parser.parse_args()
    context = run("kubectl", "config", "current-context")
    if context != EXPECTED_CONTEXT:
        raise RuntimeError(f"refusing Kubernetes context {context!r}; expected {EXPECTED_CONTEXT!r}")
    if psql(args.namespace, args.postgres_pod, "SELECT current_database()") != EXPECTED_DATABASE:
        raise RuntimeError("refusing unexpected database")
    value = manifest(args.namespace, args.postgres_pod, [args.source_id, *args.related_object_id])
    checksum = digest(value)
    print(json.dumps({"manifest_sha256": checksum, "manifest": value}, indent=2, sort_keys=True))
    if not args.execute:
        return
    if args.manifest_sha256 != checksum or args.approval != APPROVAL:
        raise RuntimeError("execute requires the current manifest hash and exact approval phrase")
    delete(args.namespace, args.postgres_pod, value)
    print(json.dumps({"deleted": True, "manifest_sha256": checksum}, sort_keys=True))


if __name__ == "__main__":
    main()
