import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const now = "2026-08-31T00:00:00Z";
const envelope = (data: unknown) => Promise.resolve(new Response(JSON.stringify({ data }), { status: 200, headers: { "Content-Type": "application/json" } }));

beforeEach(() => {
  window.history.replaceState({}, "", "/objects");
  vi.stubGlobal("fetch", vi.fn((input: RequestInfo | URL) => {
    const path = String(input);
    if (path.includes("/api/v2/runs")) return envelope([{ id: "run-1", parent_run_id: null, kind: "curator", status: "completed", actor_type: "agent", actor_id: "curator", chat_object_id: null, idempotency_key: "run-1", input: {}, trace: [], result: {}, consulted_object_ids: [], error: null, verdict: "unreviewed", review_notes: null, reviewed_by: null, reviewed_at: null, available_at: null, started_at: now, completed_at: now, created_at: now, updated_at: now }]);
    if (path.includes("/api/v2/sources")) return envelope({ items: [], next_cursor: null });
    if (path.includes("/api/v2/notes")) return envelope({ items: [], next_cursor: null });
    return envelope([]);
  }));
});

describe("minimal canonical UI", () => {
  it("has one Runs surface and no separate Curator or Evals surfaces", async () => {
    render(<App />);
    expect(await screen.findByRole("button", { name: "Runs" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Curator Runs" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Evals" })).not.toBeInTheDocument();
  });

  it("lists consolidated runs through API v2", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Runs" }));
    expect(await screen.findByText("curator run")).toBeInTheDocument();
    await waitFor(() => expect(fetch).toHaveBeenCalledWith(expect.stringContaining("/api/v2/runs"), expect.anything()));
  });
});
