# Backlog

Prioritized ideas for future work that have not yet been fully scoped. Items are
listed from highest to lowest priority.

1. **Load Context data modules from private overlays.** Let organizations extend
   an unchanged Centaur Context release with trusted, versioned, declarative data
   modules that provide bounded schema, UI, API, and agent operations. Revisit
   after the fork-based data-module approach has been implemented and evaluated.
2. **Reconsider making Enyu a direct Centaur Context fork.** Keep using the
   current Centaur Context application with the private Enyu overlay for now.
   Revisit a fork only if Enyu later needs custom schema, backend, or UI behavior
   that the standard Context API and supported extension model cannot provide.
