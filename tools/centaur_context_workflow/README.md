# Centaur Context workflow adapter

This package preserves the purpose-bound `centaur-context` RPC name used by
existing Centaur workflows. It delegates to the shared Context HTTP client and
is granted only to workflow principals. Interactive agents use the separate
`context_search`, `context_read`, and `context_apply` commands.
