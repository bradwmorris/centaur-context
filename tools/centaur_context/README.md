# Centaur Context agent client

This package provides exactly three agent-facing commands over one shared
authenticated HTTP client:

- `context_search` finds canonical Objects;
- `context_read` reads Objects and bounded supporting data; and
- `context_apply` applies one atomic, idempotent write batch.

Call them directly from an agent sandbox:

```bash
context_search 'deployment decision' --object-type task --limit 10
context_read 00000000-0000-0000-0000-000000000001 --include connections
context_apply --example
context_apply --schema
context_apply --file apply-request.json
```

`context_apply --example` is local and prints validation-only create/update
requests plus strong examples for every writable Object type. `context_apply
--schema` reads the live canonical contract without writing. Object
descriptions are current snapshots of at most 600 Unicode characters: they say
what the Object is and why it matters in the current Context. Agents never need
to inspect the generated command wrappers or Python source to use these tools.

It is designed to be loaded by a Centaur installation as an approved tool; it
is not a standalone agent runtime and never receives a database connection.

See the main [Centaur Context repository](https://github.com/bradwmorris/centaur-context)
for setup, API boundaries, and compatibility information.
