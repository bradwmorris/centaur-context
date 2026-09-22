# Contributing

Centaur Context is an early project. Before proposing a large change, open an
issue describing the user problem, the intended boundary with Centaur, and the
observable success criteria.

## Project boundaries

- Keep the core generic and organization-neutral.
- Put organization-specific agents, prompts, workflows, policies, and
  integrations in a private overlay.
- Use the authenticated HTTP API; never give an agent sandbox a database DSN.
- Do not couple Context to Centaur's operational database.
- Keep the ontology centered on canonical Objects, one-to-one subtypes,
  explained Connections, immutable Object Events, and reviewable Runs.
- Do not add public ingress or external integrations without an explicit design
  and security review.

## Development checks

Run the same checks as CI before opening a pull request:

```bash
./scripts/check-package.py
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm --prefix web ci
npm --prefix web audit --audit-level=high
npm --prefix web run type-check
npm --prefix web test -- --run
npm --prefix web run build
CONTEXT_UI_INTEGRATION=1 node --test scripts/context-ui.test.mjs
python3 -m pytest tools/centaur_context/test_client.py tools/centaur_context/test_cli.py scripts/test_rename_contract.py
python3 -m compileall -q tools/centaur_context
python3 -m pip install tools/centaur_context tools/codex_context
python3 -m pytest tools/codex_context/test_bridge.py
git diff --check
```

Database integration tests require a disposable database whose name matches a
guarded Context test-database pattern. Never point tests at a live Context or
Centaur database.

Keep pull requests focused. State what changed, why it belongs in the reusable
core, which checks passed, and any compatibility or migration impact.
