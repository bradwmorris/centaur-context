# Centaur Context agent client

This package provides exactly three agent-facing commands over one shared
authenticated HTTP client:

- `context_search` finds canonical Objects;
- `context_read` reads Objects and bounded supporting data; and
- `context_apply` applies one atomic, idempotent write batch.

It is designed to be loaded by a Centaur installation as an approved tool; it
is not a standalone agent runtime and never receives a database connection.

See the main [Centaur Context repository](https://github.com/bradwmorris/centaur-context)
for setup, API boundaries, and compatibility information.
