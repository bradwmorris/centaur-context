# Context UI modules

Context UI modules are trusted, compile-time views over canonical Context data.
They do not own a second data model and do not load third-party code at runtime.

The first registered module is the Task Board. The canonical Task list remains
the default at `/tasks`; `/tasks?view=kanban` selects the board. The registry in
`web/src/modules/moduleRegistry.tsx` declares the view and passes typed Task data,
Object visuals, and narrow update callbacks to the board. Task mutations continue
to use the authenticated, revision-checked Context API.

This is intentionally not a persisted custom-view format or a user-facing view
builder. Additional compile-time views should remain organization-neutral and
must preserve the canonical Object model and permission boundaries.
