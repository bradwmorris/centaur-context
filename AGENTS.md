# Centaur Context

Centaur Context is a reusable local-first shared context and operations application
for Centaur users and agents.

## Boundaries

- This repository owns the canonical `centaur_context` logical database and
  legacy-named `centaur_os` installations only.
- Never query or migrate Centaur's `ai_v2` or Console databases.
- Agents use the authenticated HTTP API; never give a sandbox a database DSN.
- This repository owns the standard `tools/centaur_context` agent client because the
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

## Git Workflow

Treat the commit on `origin/main` as the canonical landed checkpoint. A local
commit, pushed feature branch, merged pull request, and synchronized local
`main` are different states; never describe work as landed or synchronized
without checking the relevant state directly.

### Starting Work

- Use the canonical checkout at
  `/Users/bradleymorris/Desktop/dev/centaur-context` for `main`.
- Before editing `main`, require a clean working tree and run
  `git pull --ff-only`. If `main` is dirty or cannot fast-forward, stop and
  preserve the existing work; do not overwrite, discard, or silently mix it
  with the new job.
- An obvious, self-contained small change may be committed and pushed directly
  on `main` when that is consistent with the task's approval boundaries.
- Use a branch or worktree for anything requiring multiple commits, meaningful
  investigation, an RD, review, or coordination. Create it from the latest
  `origin/main`, keep one job per branch, and make the branch track its matching
  remote feature branch rather than `origin/main`.

### Landing Work

- Commit and verify the scoped work before pushing.
- For branch work, push the feature branch and use a pull request. Merge only
  when the applicable task and approval rules authorize it.
- A remote merge does not update any local checkout. Immediately after a merge,
  fast-forward the canonical local `main` with:

  ```bash
  git -C /Users/bradleymorris/Desktop/dev/centaur-context pull --ff-only
  ```

- Confirm that `git rev-parse main` and `git rev-parse origin/main` return the
  same commit and that the canonical checkout is clean.
- After the merge and synchronization are verified, remove the completed
  worktree and local feature branch when safe. Retain them only for an explicit
  reason and report that reason.

### Handoff Checkpoint

End every Git-changing task with a compact checkpoint containing:

```text
LANDED or NOT LANDED
PR: <number or n/a>
origin/main: <commit>
local main:  <commit>
canonical checkout: clean or dirty
feature worktree: removed, retained with reason, or n/a
```

Use `LANDED` only when the intended commit is present on `origin/main`. Use
`NOT LANDED` when work exists only in a working tree, local commit, remote
feature branch, or unmerged pull request, and state exactly where it remains.

## Verification

Run before handing off changes:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm --prefix web run type-check
npm --prefix web run build
python3 -m pytest tools/centaur_context/test_client.py
python3 -m compileall -q tools/centaur_context
```

Database integration tests require `TEST_DATABASE_URL` and must target a
disposable database whose name contains `centaur_context_test` or the legacy
`centaur_os_test` pattern.
