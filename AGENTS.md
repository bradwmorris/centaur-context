# Centaur OS

Centaur OS is a reusable local-first shared context and operations application
for Centaur users and agents.

## Boundaries

- This repository owns the `centaur_os` logical database only.
- Never query or migrate Centaur's `ai_v2` or Console databases.
- Agents use the authenticated HTTP API; never give a sandbox a database DSN.
- This repository owns the standard `tools/centaur_os` agent client because the
  client is part of the public API contract.
- Organization-specific agents, prompts, workflows, retention choices, and
  business rules belong in that organization's private overlay.
- Keep the ontology centred on canonical Objects, with Tasks, Chats, Users,
  Entities, and event-shaped Memories as one-to-one subtype records, explained
  Connections, and immutable Object Events.
- Do not copy code, credentials, data, or schema wholesale from The AGI Post.
- Do not add public ingress, cloud deployment, or external integrations without
  the repository owner's explicit approval.

## Development Jobs

Before planning or executing a requirements document, read `dev/AGENTS.md` and
the relevant file in `dev/rd/`. Planning and execution are separate modes.

## Verification

Run before handing off changes:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm --prefix web run type-check
npm --prefix web run build
python3 -m pytest tools/centaur_os/test_client.py
python3 -m compileall -q tools/centaur_os
```

Database integration tests require `TEST_DATABASE_URL` and must target a
disposable database whose name contains `centaur_os_test`.
