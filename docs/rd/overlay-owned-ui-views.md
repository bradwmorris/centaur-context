# RD: Overlay-owned UI views without core source modifications

GitHub Issue: [#63](https://github.com/bradwmorris/centaur-context/issues/63)

Status: v1 design finalized on 2026-09-22 at the owner’s request. Implementation authorized separately on 2026-09-22 and tracked by #63. Private overlay deployment remains separate.

## Outcome

An adopter can clone an unchanged Centaur Context release, define custom UI views in a separate overlay repository, and build a UI that discovers and displays those explicitly enabled views. With no overlay configured, Context retains its existing behavior and builds independently.

The reusable core owns the extension interface, navigation, canonical data access, and generic UI primitives. The overlay owns custom layouts, presentation rules, dependencies, and deployment selection. This supports a maintainable app-extension boundary for potential upstream adoption; it does not claim endorsement or stock upstream Centaur compatibility.

## Evidence and related work

Inspected Context revision: `8d747125b67cbd95bedd7a3891373703744b3dd4`.

- `web/src/modules/moduleRegistry.tsx` statically imports TaskBoard and registers `kanban`. Its context exposes Tasks, visuals, loading, a React state setter, and reload.
- `web/src/App.tsx` resolves registered views and renders the switcher in collection pages. Routing and available sections are currently fixed.
- `web/src/modules/taskBoard/TaskBoard.tsx` imports internal API, types, and routing directly. Merely moving that file outside the repository would not create a supported extension boundary.
- `web/vite.config.ts` and `Dockerfile` currently build only the in-repository frontend.
- `docs/ui-modules.md` explicitly describes trusted compile-time modules and excludes runtime third-party code loading.
- A separately inspected private deployment recipe rebuilds the public frontend; it does not supply external view modules. Private paths, identities, deployment pins, and configuration are deliberately omitted here.
- #15 delivered the built-in Board. #9 concerns data-module extension conventions, including migrations and backend registration. This issue owns external UI-only composition and must coordinate the UI seam with #9 without taking on its data-model scope. #11 concerns startup ergonomics.

## Finalized decision

Use **explicit build-time composition of trusted local view packages** for v1. A deployment configuration lists packages or directories outside the core checkout; supported build tooling validates their manifests and generates a registry in disposable build output. Never patch App.tsx, copy overlay files into tracked core directories, scan unrelated directories, or execute extension code merely because it exists on disk.

Adding, changing, or removing a view requires rebuilding and redeploying the frontend. This is an intentional v1 constraint, distinct from editing core source. A running container cannot discover new TSX files on a host automatically.

Selected overlay layout (to be implemented):

```text
my-overlay/
  context-ui/
    views.config.json
    package.json
    package-lock.json
    views/
      my-task-board/
        manifest.json
        index.tsx
        styles.module.css
```

### Manifest and registration

Specify a versioned manifest with a namespaced stable view ID, display label, supported host API version, existing target section, entry point, and deterministic ordering. Configuration explicitly selects enabled views. Reject duplicate IDs, collisions with built-in IDs, unsupported versions/sections, missing entries, and paths escaping the configured extension root. Namespaced IDs are URL encoded in the existing `?view=` convention.

Support only the existing Tasks collection-view slot in v1. Require a typed Tasks adapter and an external Task view as the executable proof. Reject every other section with an actionable build error. Do not imply that the current Tasks-only context is a universal data interface. Arbitrary sidebar entries, new routes, and record-detail panels are future extension points.

### Public host interface

Expose one documented import surface for canonical types, navigation to existing Object/detail routes, supported presentation tokens/primitives, and typed data/mutation operations. External code must not import private `web/src/*` modules or receive raw App state setters.

The host owns confirmed-data reconciliation and refresh/invalidation. Task writes use the existing canonical HTTP operations, expected revisions, current idempotency semantics, validation, and Event/Run attribution. Views must handle loading, errors, stale revisions, empty results, and the documented query/pagination completeness of their adapter. Changing presentation must not invent Task statuses or another Task store.

Define compatibility for the view manifest, host UI interface, and backend API used by the resulting bundle. Initially an exact supported host release is acceptable if clearly documented; do not promise broad semver compatibility without tests. Keep a single compatible React runtime and reproducible dependency resolution across host and overlay.

### Trust and resilience

Enabled modules are trusted application code in the same browser origin. The host interface is a maintainability boundary, not a sandbox or enforceable per-view permission system. A module can make browser requests with the same access as the host; a manifest cannot remove that authority.

Preserve the existing human-listener deployment boundary. Inspection shows `human_router` applies human actor attribution without the agent bearer-token middleware; do not describe this browser surface as providing agent-style authentication. Agent tooling continues to use authenticated HTTP APIs. Do not place database DSNs, workflow tokens, or other service credentials in frontend configuration or bundles.

Use scoped styles and a host error boundary with a return-to-list action. These contain ordinary presentation failures; they do not isolate malicious code, global effects, or an infinite loop. Invalid configured extensions fail the build with actionable diagnostics. A removed/unknown view URL falls back to the canonical list with a useful notice. Extension assets ship locally with the frontend; no remote script discovery, marketplace, public ingress, or external integration is added.

### Packaging and default experience

Provide documented local development and container-build entry points that accept an explicit overlay configuration and leave the core checkout unchanged. Use clean temporary staging/generated files; resolve only selected packages. Development file access must be restricted to configured roots and must not expose the whole overlay repository or secrets as static assets.

Record core revision, enabled module versions/source revisions, lockfiles, and expected backend compatibility in build metadata. Prove removal and rollback by rebuilding without the overlay configuration; canonical data remains intact.

Keep the generic built-in Board enabled by default and preserve `/tasks?view=kanban`. Adapt it to the same public interface so core and external views do not grow competing APIs. Neutrality does not require removing a useful generic view. Prove external ownership with a small synthetic Task view in a separate fixture/package, not a copy of the core application in an overlay.

## Scope and delivery sequence

1. Design finalized by this revision. Begin implementation only in a separate execute-issue run for #63, using the contract below.
2. Implement the minimal host interface and adapt the built-in Board without changing its behavior.
3. Implement explicit external package composition, manifest checks, development/build integration, diagnostics, and error recovery.
4. Add a synthetic external Task view and an adopter walkthrough covering enable, build, upgrade, disable, and rollback from a clean core checkout.
5. Verify the contract and documented checks, self-review, and open a linked PR through the repository issue workflow.

Actual private overlay views and deployment migration belong in separately tracked private work after the public interface is available. Do not copy private configuration or core product code into that overlay. This public issue does not authorize that deployment.

## Non-goals

- Runtime installation/hot-loading of arbitrary JavaScript, untrusted plugin sandboxing, module federation, or an extension marketplace.
- A no-code view builder, persisted per-user layouts, or a general dashboard framework.
- New Objects/subtypes, migrations, backend plugins, custom Task statuses, or a second datastore; #9 remains separate.
- Removing the built-in Board, replacing canonical detail pages, or arbitrary application-shell overrides.
- Changes to base Centaur, its operational database, public networking, external integrations, or live data.

## Acceptance checks

- [ ] A fresh checkout at a supported release builds and passes existing UI tests with no overlay or private repository present; default List/Board behavior and existing URLs remain intact.
- [ ] From a separate synthetic overlay directory, enable a namespaced Task view using only documented public imports/configuration. It appears in Tasks, reads canonical data, opens canonical details, and performs a revision-checked status change through the host interface.
- [ ] Build and development startup leave tracked core files unchanged. No copied product source or manual registry/App patch is needed; a second external view can be added solely in the overlay.
- [ ] Local and container builds both support the documented composition. Dependency locks, one React runtime, generated registry handling, assets, and core/module/backend compatibility are verified.
- [ ] Duplicate/reserved IDs, unsupported host versions/sections, missing packages/entries, disabled packages, and invalid paths have deterministic tested behavior. An explicitly broken extension fails the build rather than silently disappearing.
- [ ] View selection survives reload, back, and forward. Disabled/removed views recover to the canonical list; ordinary render failures expose a recovery path without taking down the application shell.
- [ ] Loading, empty data, query/pagination semantics, scoped styling, keyboard navigation, and error/conflict behavior are covered. Unconfirmed mutations do not update canonical UI state; list, board, and detail agree after successful writes.
- [ ] Built-in Board behavior remains covered, including blocked-reason entry/clearing and completion semantics. No permission, revision, validation, or Event/Run guarantees are weakened.
- [ ] Public documentation explains the trusted-code model, build/redeploy requirement, supported extension slot, compatibility policy, and absence of credential embedding or database access.
- [ ] Tests confirm the dev/static asset boundary does not expose unrelated overlay files. The public example contains synthetic content only.
- [ ] Rebuilding without custom views restores the stock experience without data migration or deletion. Repeating the walkthrough against a subsequent supported core revision requires no core patches.
- [ ] Run all checks in CONTRIBUTING.md for implementation, with database integration tests restricted to a disposable database whose name contains `centaur_context_test`.

## Final v1 contract

The following names and commands are implementation targets, not capabilities already shipped.

### Public interface and compatibility

- Public import: `@centaur-context/ui`. Provide a host-owned TypeScript export surface with `TaskViewProps`, canonical Task/read-only visual types, supported presentation primitives and navigation helpers. It is a local build export, not a required public npm publication.
- Entry points default-export a React component accepting `TaskViewProps`. The host supplies read-only results, loading/error/completeness information, reload, canonical Object navigation and a revision-aware Task status mutation. No arbitrary HTTP client, raw React state setters or new mutation authority is added to the public interface.
- The host adapter retains the existing Task query semantics and explicitly reports any limit/partial result. It never represents a bounded result as all Tasks. Status writes preserve blocked-reason entry/clearing, completion semantics and the ownership/brief rules delivered by #66; reconcile confirmed server responses across views and detail pages.
- Manifest schema and host UI API version start at integer `1`. Every enabled view pins one full immutable core Git SHA, not a moving branch or a broad version range. Build and development startup reject a mismatch. Manifest/backend metadata records API `v2`; deployment must pair the bundle with the backend from that core revision. The pin is a compatibility check, not a signature or trust guarantee.
- The host owns the single React/React DOM runtime, including JSX runtime imports. Reject incompatible peers or extension runtime dependencies that introduce another React copy. Preserve default List and built-in `kanban`, adapting Board to this same host interface.

### Configuration and manifest

`context-ui/views.config.json` is strict JSON:

```json
{
  "schemaVersion": 1,
  "views": ["views/example-task-view/manifest.json"]
}
```

Only listed manifests are enabled; an empty list means stock views. The configuration directory is the extension root. Paths must be relative and their real paths must stay within that root, including after symlink resolution. Validate JSON without importing extension code. Unknown fields and malformed values fail with a file/field diagnostic.

Each view manifest has this shape (the revision value below is illustrative):

```json
{
  "schemaVersion": 1,
  "id": "example:task-summary",
  "label": "Task summary",
  "hostApiVersion": 1,
  "coreRevision": "0123456789012345678901234567890123456789",
  "section": "tasks",
  "version": "1.0.0",
  "entry": "index.tsx",
  "files": ["index.tsx", "styles.module.css"],
  "order": 100
}
```

Manifest entry/files are relative to its directory and cannot escape it. `files` is an exact list of selected source/assets, not glob patterns or a directory walk. Require the entry to be listed; reject secret/config files such as `.env`, escaping symlinks, duplicate files/IDs, reserved built-in IDs, unsupported sections/versions and missing files. An ID uses lowercase namespace/name tokens separated by `:`; URL-encode it in `?view=`. Order by numeric `order`, then ID. Disabled/unlisted views are not loaded. Extension-relative imports must resolve only to staged listed files or locked dependencies, never private core files or unrelated overlay directories.

### Dependencies and staging

One `package.json` and npm `package-lock.json` at the extension root own all overlay dependency resolution. Require both for a configured overlay, even with no extra dependencies. The core retains its existing independent lockfile. Install from both locks without rewriting either; fail on lock drift. No implicit network lookup for an unlisted local view and no wildcard local workspace discovery. V1 rejects local-path/git dependencies outside the selected source set; use locked registry dependencies. Installation must not execute overlay dependency lifecycle scripts; packages requiring such setup are unsupported in v1 with an explicit diagnostic.

Create a disposable staging workspace outside the core and overlay checkout. Stage core build inputs, exact selected extension files and sanitized dependency manifests/locks; generate the registry there. Treat dependency code as trusted application code. Resolve the public host export and React family to the host runtime; ordinary overlay dependencies resolve through the overlay lock. Type-check external components as part of the build, not merely transpile them.

Development serves only the staging workspace and required installed dependencies, with strict filesystem allowlisting and no broad overlay static/public directory. Do not enable Vite's default environment loading against the overlay root. Carry only explicitly supported non-secret build configuration. Test denial of unrelated overlay files, traversal, escaping symlinks and environment files through both imports and dev HTTP requests. Error boundaries and scoped CSS contain ordinary failures, not malicious modules.

### Build commands and metadata

Add one Node entry point at `scripts/context-ui.mjs`:

```sh
node scripts/context-ui.mjs dev --config /absolute/path/context-ui/views.config.json
node scripts/context-ui.mjs build --config /absolute/path/context-ui/views.config.json --out-dir /absolute/path/output
node scripts/context-ui.mjs image --config /absolute/path/context-ui/views.config.json --tag context-with-views:test
```

Omitting `--config` selects the stock UI. `build` requires an explicit output directory and refuses destructive replacement of unrelated/nonempty output. `dev` keeps its staging workspace only for the session and uses the existing API-target setting against an isolated test API during verification. Existing stock npm and image-build commands continue to work unchanged for adopters without an overlay.

`image` constructs a sanitized temporary Docker context from core inputs plus selected extension build inputs. The frontend compiles inside the container using the same manifest checks, pins and locks as local builds. Never send the original overlay repository as Docker context. Preserve the existing backend build and runtime behavior. Share composition logic across commands; do not maintain separate handwritten registries.

Emit a non-secret build metadata artifact containing core SHA, manifest/host API versions, expected backend API, selected view IDs/versions, overlay source revision when available, selected-source content hashes and both lockfile hashes. Do not publish absolute host paths or secrets. Record a content hash even for dirty/non-Git overlay sources. For release verification use a committed core revision; development dirty-core builds must be visibly marked and must not claim exact-release compatibility.

### Execution and integration boundaries

Implement in one isolated `codex/63-overlay-ui-views` worktree. Primary scope is `web/`, the new build entry point, container build integration, synthetic fixtures and UI/RD documentation. No database migrations or backend contract changes are required. #9 reuses this UI interface when its separate data-module design proceeds; #11 can wrap these commands later without this issue taking on the general startup lifecycle.

Coordinate integration with active #77 maintenance and #78 memory work; refresh main and resolve any shared build/version changes before final checks. Run database tests only on a dedicated disposable database containing `centaur_context_test` in its name. Keep browser verification on synthetic data and separate ports. Serialize heavy container builds when local disk is constrained; do not prune another task's data/images or use the live deployment as a test target.

Deliver a synthetic external Task view, the unchanged default experience, local/container proofs, removal/rollback proof and an upgrade walkthrough. Use two committed core revisions for the upgrade exercise when available; otherwise explicitly leave the subsequent-release acceptance check unproven rather than fabricate it. Follow the repository merge rule: missing material acceptance evidence must be reported before claiming completion. Private overlay adoption and deployment remain separate work.
