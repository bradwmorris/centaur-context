# Issue 144 browser verification

The screenshots use a browser-only synthetic fixture served by the Vite preview on `127.0.0.1:5173`; no shared or private task data was loaded. The fixture includes overdue, today, tomorrow, later, missing, malformed, and completed tasks.

- `due-date-desktop.png` shows the Due date view at 1440 × 1000 with completed tasks enabled. It shows the overdue, today, tomorrow, exact-day, no-due-date, and final Completed groups; the malformed value stays visible as “Date unavailable”.
- `due-date-narrow.png` shows the same list at 390 × 844. The collection toolbar scrolls horizontally. Tabbing from the sort select moved focus to List and scrolled the toolbar from `scrollLeft=0` to `255`; its maximum scroll was `333`, exposing the remaining controls.

In the synthetic fixture, I opened “Review synthetic task data”, changed its Due value from 13 October 2026 at 17:00 to 14 October at 17:00, and blurred the field. The browser sent `PATCH /api/v2/tasks/preview-today` with `due_at: 2026-10-14T06:00:00.000Z`. Returning to Tasks moved it into Tomorrow. The task list requests used `sort=recent`; selecting Due date did not send an unsupported sort value. Reloading `/tasks` loaded the list again. Unit tests cover recomputing at local midnight and after a tab resumes.
