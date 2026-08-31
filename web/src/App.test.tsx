import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const now = "2026-08-31T00:00:00Z";
const envelope = (data: unknown) => Promise.resolve(new Response(JSON.stringify({ data }), { status: 200, headers: { "Content-Type": "application/json" } }));
const source = { id: "source-1", kind: "source", title: "Ryan Greenblatt YouTube video", description: "A transcript-backed Source.", provenance: {}, revision: 1, protected: true, created_at: now, updated_at: now };
const chat = { id: "chat-1", kind: "chat", title: "Slack channel conversation", description: "The originating conversation.", provenance: {}, revision: 1, protected: true, created_at: now, updated_at: now };
const users = [
  { user_object_id: "brad-1", title: "Brad", user_kind: "human", role: "participant", avatar_url: null, avatar_asset_url: null },
  { user_object_id: "rez-1", title: "Rez", user_kind: "agent", role: "participant", avatar_url: null, avatar_asset_url: null },
];

beforeEach(() => {
  window.history.replaceState({}, "", "/objects");
  vi.stubGlobal("fetch", vi.fn((input: RequestInfo | URL) => {
    const path = String(input);
    if (path.includes("/api/v2/connection-graph")) return envelope({ fingerprint: "graph", node_count: 0, connection_count: 0, nodes: [], edges: [] });
    if (path.includes("/api/v2/runs")) return envelope([{ id: "run-1", parent_run_id: null, kind: "intake", status: "completed", actor_type: "centaur_agent", actor_id: "workflow-enyu-source-ingestion", chat_object_id: chat.id, primary_object_id: source.id, idempotency_key: "run-1", input: {}, trace: [], result: { counts: { objects: 0, connections: 6 }, object_ids: [source.id] }, consulted_object_ids: [], error: null, verdict: "unreviewed", review_notes: null, reviewed_by: null, reviewed_at: null, available_at: null, started_at: now, completed_at: now, created_at: now, updated_at: now }]);
    if (path.includes("/api/v2/object-visuals")) return envelope([{ object_id: chat.id, source_provider: "slack", users }]);
    if (path.includes("/api/v2/objects?")) return envelope([source, chat]);
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
    expect(await screen.findByText("Source ingestion · Ryan Greenblatt YouTube video")).toBeInTheDocument();
    expect(screen.getByText("Reused Source · Added 6 connections")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Brad, Human, participant" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Rez, Agent, participant" })).toBeInTheDocument();
    await waitFor(() => expect(fetch).toHaveBeenCalledWith(expect.stringContaining("/api/v2/runs"), expect.anything()));
  });

  it("opens Connections as a first-class active navigation surface", async () => {
    render(<App />);
    const button = await screen.findByRole("button", { name: "Connections" });
    fireEvent.click(button);
    expect(window.location.pathname).toBe("/connections");
    expect(button).toHaveAttribute("aria-current", "page");
    expect(await screen.findByRole("region", { name: "Connections graph" })).toBeVisible();
  });
});
