# Context UI views

The default Tasks List and Board work without an overlay. `/tasks?view=kanban`
continues to select Board. External views are explicitly selected **trusted
application code**, compiled into the same frontend. They are not sandboxed and
have the same browser access as the host. Adding/removing a view requires a build
and, for an installed service, a separate deployment.

## Try an independently owned view

Use Node 22 and npm. Start from a committed core revision containing this feature.
Create the synthetic example in a **new directory outside the core checkout**:

```sh
node scripts/create-ui-example.mjs /tmp/my-context-views
node scripts/context-ui.mjs build --config /tmp/my-context-views/views.config.json --out-dir /tmp/my-context-web
CENTAUR_CONTEXT_DEV_API_TARGET=http://127.0.0.1:8080 node scripts/context-ui.mjs dev --config /tmp/my-context-views/views.config.json --port 5193
node scripts/context-ui.mjs image --config /tmp/my-context-views/views.config.json --tag context-with-views:test
```

The API target must be the intended human listener. Use synthetic data and an
isolated API for development. Open `/tasks?view=example%3Atask-summary`; Summary
reads Tasks, opens canonical detail and moves a To do task to In progress using
its expected revision. It is a demonstration view, not another task datastore.
The image command builds only; it never deploys. Output directories must not
already exist. `dev` binds loopback on its chosen port and refuses a busy port.

Composition takes a snapshot in a temporary directory and removes it on normal
exit. Restart the command after changing overlay files; v1 does not watch/copy
changes from the original overlay. Core Git-tracked build inputs are copied,
including modified tracked files for development. Add new core development files
to Git before testing them. Dirty core builds are explicitly marked in metadata;
release builds must use a clean committed core revision. No composition command
modifies either checkout or their lockfiles. Stock `npm --prefix web run build`
and `scripts/build-image.sh` remain supported.

## Contract

See the [final RD](rd/overlay-owned-ui-views.md) for strict JSON examples and
acceptance checks. `views.config.json` schema 1 lists manifest paths relative to
its directory. Unlisted manifests are disabled and never read. Each manifest
selects `section: "tasks"`, host API 1, a namespaced ID, label, three-part version,
integer order, full matching core Git SHA, entry and exact list of source/assets.
Paths must remain within the configured root after symlink resolution. Missing
files, unknown fields, duplicate/reserved IDs, unsupported sections/versions and
escaping paths fail explicitly. Selected files may be TS/TSX, JS/JSX, CSS, SVG,
PNG/JPEG/WebP or WOFF fonts. Use CSS modules; global effects are not isolated.

Entry points default-export a React component with `TaskViewProps` imported from
`@centaur-context/ui`. The host exports canonical Task types, Task identity
presentation components and canonical route helpers through that single surface.
Never import private `web/src` modules. The built-in Board uses the same adapter.

The adapter supplies:

- Read-only Tasks/visuals, loading/error state, and completeness. The existing
  client follows every page of active Tasks at 100 rows per request. `complete`
  means those pages finished successfully, not a transactionally stable snapshot.
  Failed loads may leave previous rows visible and report `unknown` completeness.
- `reload()` and `openTask(id)`; canonical detail remains host-owned.
- `changeStatus(task, status, blockedReason?)`, which writes the supplied expected
  revision, requires a blocked reason on entry, clears it on exit, and retains
  the current API's idempotency, ownership and completion rules. Rejected writes
  throw; do not present an optimistic local mutation as confirmed. The host
  reconciles the server response and refreshes the collection. It never exposes
  a raw state setter or a database credential.

Browser calls retain the existing human-listener deployment boundary; that
listener is not agent bearer-token authentication. Agent access remains on the
separate authenticated API. This interface is a maintainability contract, not an
enforceable permission boundary for trusted JavaScript.

## Dependencies, reproducibility and file access

The overlay root must contain `package.json` and npm lockfile version 3, even
without dependencies. Extra packages belong in `dependencies`; the lock must be
consistent and registry artifacts must have integrity hashes. V1 supports locked
npm registry packages only, not local/git/workspace dependencies, overlay scripts,
dev/optional dependencies or packages requiring install scripts. The install uses
`npm ci --ignore-scripts`. React and React DOM are provided by the host; if declared
as peers, their versions must exactly match the host. Runtime aliases, including
JSX imports, ensure one host React runtime.

The host lock and overlay lock remain separate. Staging includes only listed view
files and the validated package/lock data; `.npmrc`, `.env`, unrelated overlay
files and original overlay directories are never copied or served. External code
must import its selected local files, the public host interface or locked packages.
Vite's strict filesystem boundary allows the staged web directory only, and
composed builds do not load overlay environment files or inherit `VITE_*` values.
Avoid secret literals in selected source: trusted code and its assets become public
browser content. Host error boundaries offer Return to list for ordinary render
failures; they cannot contain infinite loops or malicious code.

Local and container frontend builds use the same generated registry and lockfiles.
`context-build.json` records the core revision/dirty flag, API versions, enabled
view IDs/versions, selected-source hashes, overlay revision when available, and
lockfile hashes without machine paths. Serve the bundle with the backend from
that core revision; backend API metadata is `v2`, not a promise that every `v2`
backend release is compatible.

## Upgrade, disable and rollback

For an upgrade, keep the overlay outside the checkout, update core, review public
interface changes, and update each enabled manifest's `coreRevision` to the new
full SHA. Resolve any explicit compiler/compatibility errors through public imports,
then repeat type-check/build and the read/detail/status walkthrough. No core source
patch or registry edit is needed. Keep old core revision, overlay source and both
locks together for reproducible rollback.

To disable a view, remove its manifest from `views.config.json` and rebuild. An
unknown/removed `?view=` renders the canonical list with a notice. To remove the
whole overlay, omit `--config` and rebuild to a new output directory or image tag.
Canonical data needs no migration or deletion. Re-deploying a previous verified
bundle/backend pair is a separate deployment operation.
