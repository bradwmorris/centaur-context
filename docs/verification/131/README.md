# Adoption documentation evidence

Issue [#131](https://github.com/bradwmorris/centaur-context/issues/131) follows the
[revised adoption RD](../../rd/context-adoption-readiness.md). The owner narrowed
the work on 2 October 2026 to an adaptable proof-of-concept guide and essential
configuration corrections. Full installation certification, real Slack/model
tests, outage tests, and repeated backup/restore exercises are outside that
completion boundary. Unperformed checks have not become passes.

## What the evidence establishes

| Evidence | What it supports | Limit |
| --- | --- | --- |
| Existing-installation experience | Historical Slack capture/retrieval and subscription transport informed the integration design. | An existing customized installation does not certify these public examples or a fresh paired release. |
| Source and configuration review | API/auth/Task/Note rules, model names, public fork hooks, script arguments, labels, and network paths match the reviewed source. | Static agreement does not prove a deployment or model works. |
| [Disposable component receipt](step5.md) | Enforced DNS/network paths, authenticated Context API, synthetic capture/replay, Fact/Task writes, retrieval, and UI. | Caller pods simulated Slackbot/proxy labels; no real Slack/model loop. |
| [Build and broker follow-up](step5-followup.md) | Source-linked Context image build succeeded using matching cached layers; a supported broker request was attempted. | Model execution failed at proxy-image startup. No successful extraction, later Memory retrieval, or paired version pin. |

The broker finding remains historical operational evidence. Repairing that
installation or adding a separate Slack app is not part of this change.

## Documentation resolution map

| RD finding | Resolution |
| --- | --- |
| 1. Automatic curation | README loop and Memory guidance distinguish Chat capture, append-only extraction, optional maintenance, and explicit writes. |
| 2. Normal agent writes | Setup and contract describe search/read/apply on authenticated 8081; retained 8084 and specialist credentials remain distinct. |
| 3. Tasks | API and contract specify canonical User owner, due date, code Issue URL, and human/legacy differences. |
| 4. Notes | Schema/API/contract use Idea, Excerpt, Fact and document legacy stored values without migration or aliases. |
| 5. Network example | Slackbot retrieval ingress is included; opt-in peer policies and matching Centaur values explain both sides of each path. |
| 6. Accessible fork guidance | Setup and integration source map explain required behavior with public references; no unpublished adjacent fork guide is required. |
| 7. Model compatibility | Examples use exact `gpt-6-luna`; explicit legacy configuration needs a matching broker. Model access is optional. |
| 8. Navigation | README links Codex, Memory, Research Artifacts, and advanced operations. |
| 9. Adaptation/evidence | Setup begins with existing Centaur, creates a separate Context database, and distinguishes source checks, component results, and historical installation experience. |

## Maintenance review

Commands were cross-checked against `bootstrap-database.sh`, its SQL file,
`backup.sh`, `restore.sh`, `validate-backup-metadata.py`, `install-kubernetes.sh`,
`uninstall-kubernetes.sh`, `drop-database.sh`, and `common.sh`:

- Bootstrap creates the guarded database and pgvector, assigns its application
  role, and does not rotate an existing role's password.
- Backup preserves the dump, checksum, and metadata; it does not preserve
  Kubernetes Secrets or deployment configuration. Private legacy-name consent
  is a backup-only exception.
- Restore requires a prepared target and exact name confirmation, validates
  sidecars, preserves extension setup, and runs in a transaction. Writers must
  be stopped and recovery checked before switching consumers.
- Upgrade uses the reviewed image and forward migrations; changing the image
  alone is not a database rollback. Model checks apply only when enabled.
- Uninstall retains the database and Secret by default, and does not remove
  separately applied peer policies. Database deletion is separate; an application
  role still used by a retained restore database must be kept.

This is a script/documentation review, not a new live upgrade or restore receipt.
The existing rename/guard tests cover database-name boundaries, backup metadata,
and uninstall Service coverage. This checkpoint passed all 13 tests, contract
generation and API-doc checks, 99 local link/heading checks, compatibility TOML
parsing, maintenance shell syntax checks, and `git diff --check`. The values
example changed only comments; its settings are unchanged. The diff was
self-reviewed against the scripts and revised RD.

Required final CI and PR review remain for the delivery checkpoint; the earlier
component tests need not be repeated.
