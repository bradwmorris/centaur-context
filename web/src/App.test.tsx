import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
const run = { id: "run-1", parent_run_id: null, kind: "intake", status: "completed", actor_type: "centaur_agent", actor_id: "workflow-enyu-source-ingestion", chat_object_id: chat.id, primary_object_id: source.id, idempotency_key: "run-1", input: {}, trace: [], result: { counts: { objects: 0, connections: 6 }, object_ids: [source.id] }, consulted_object_ids: [], error: null, verdict: "unreviewed", review_notes: null, pinned: false, reviewed_by: null, reviewed_at: null, available_at: null, started_at: now, completed_at: now, created_at: now, updated_at: now };

beforeEach(() => {
  window.history.replaceState({}, "", "/objects");
  vi.stubGlobal("fetch", vi.fn((input: RequestInfo | URL) => {
    const path = String(input);
    if (path.includes("/api/v2/connection-graph")) return envelope({ fingerprint: "graph", node_count: 3, connection_count: 2, nodes: [{ id: source.id, kind: "source", title: source.title }, { id: chat.id, kind: "chat", title: chat.title }, { id: "other-1", kind: "entity", title: "Other" }], edges: [{ id: "edge-1", source_object_id: chat.id, target_object_id: source.id, kind: "about", description: "Chat is about source" }, { id: "edge-2", source_object_id: chat.id, target_object_id: "other-1", kind: "involves", description: "Chat involves other" }] });
    if (path.includes("/api/v2/runs")) return envelope([run]);
    if (path.includes("/api/v2/object-visuals")) return envelope([{ object_id: chat.id, source_provider: "slack", users }]);
    if (path.includes("/api/v2/objects?")) return envelope([source, chat]);
    if (path.includes("/api/v2/sources")) return envelope({ items: [], next_cursor: null });
    if (path.includes("/api/v2/notes")) return envelope({ items: [], next_cursor: null });
    return envelope([]);
  }));
});

describe("minimal canonical UI", () => {
  it("keeps Runs and adds Evals as a bottom navigation surface", async () => {
    render(<App />);
    expect(await screen.findByRole("button", { name: "Runs" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Evals" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Curator Runs" })).not.toBeInTheDocument();
  });

  it("shows all root Runs in an editable pinned-first eval table", async () => {
    const defaultFetch = vi.mocked(fetch).getMockImplementation()!;
    const pinned = { ...run, id: "run-golden", idempotency_key: "run-golden", kind: "slack_interaction", primary_object_id: null, input: { title: "Golden prompt" }, result: { summary: "Golden result" }, pinned: true, created_at: "2026-08-30T00:00:00Z" };
    const newer = { ...run, id: "run-newer", idempotency_key: "run-newer", kind: "slack_interaction", primary_object_id: null, input: { title: "New prompt" }, result: { summary: "New result" }, created_at: "2026-09-01T00:00:00Z" };
    vi.mocked(fetch).mockImplementation((input, init) => {
      const path = String(input);
      if (path.includes("/runs/run-newer/review")) return envelope({ ...newer, pinned: true, result: { ...newer.result, review_revision: 1 } });
      if (path.includes("/api/v2/runs?") && path.includes("pinned=true")) return envelope([pinned]);
      if (path.includes("/api/v2/runs?") && path.includes("pinned=false")) return envelope([newer]);
      return defaultFetch(input, init);
    });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Evals" }));
    expect(await screen.findByRole("table", { name: "Eval runs" })).toBeVisible();
    expect(screen.getByRole("columnheader", { name: "Actual result" })).toBeVisible();
    expect(screen.queryByRole("columnheader", { name: "Input" })).not.toBeInTheDocument();
    expect(await screen.findByText("Golden result")).toBeVisible();
    const rows = screen.getAllByRole("row");
    expect(rows[1]).toHaveTextContent("Golden prompt");
    expect(rows[2]).toHaveTextContent("New prompt");
    expect(screen.getByRole("columnheader", { name: "Users" })).toBeVisible();
    expect(within(rows[1]).getByRole("img", { name: "Brad, Human, participant" })).toBeVisible();
    expect(within(rows[1]).getByRole("img", { name: "Rez, Agent, participant" })).toBeVisible();
    expect(within(rows[1]).getByRole("button", { name: /Golden prompt/ })).toHaveAttribute("title", "Open Bot interaction · Golden prompt");
    expect(screen.getByRole("button", { name: "Edit annotation for run-golden" })).toBeVisible();
    await waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(expect.stringMatching(/\/api\/v2\/runs\?.*root_only=true.*pinned=true/), expect.anything());
      expect(fetch).toHaveBeenCalledWith(expect.stringMatching(/\/api\/v2\/runs\?.*root_only=true.*pinned=false/), expect.anything());
    });

    fireEvent.click(screen.getByRole("button", { name: "Pin run-newer as a golden eval" }));
    await waitFor(() => {
      const reviewCall = vi.mocked(fetch).mock.calls.find(([input]) => String(input).includes("/runs/run-newer/review"));
      expect(reviewCall).toBeDefined();
      expect(JSON.parse(String(reviewCall?.[1]?.body))).toMatchObject({ pinned: true, verdict: "unreviewed", expected_revision: 0 });
    });
    expect(await screen.findByRole("button", { name: "Unpin run-newer from golden evals" })).toBeVisible();
  });

  it("fails visibly when the backend does not support eval pins", async () => {
    const defaultFetch = vi.mocked(fetch).getMockImplementation()!;
    const { pinned: _pinned, ...legacyRun } = run;
    vi.mocked(fetch).mockImplementation((input, init) => {
      const path = String(input);
      if (path.includes("/api/v2/runs?")) return envelope([legacyRun]);
      return defaultFetch(input, init);
    });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Evals" }));
    expect(await screen.findByText(/Evals API is out of date/)).toBeVisible();
    expect(screen.getByRole("table", { name: "Eval runs" })).toBeVisible();
    expect(screen.queryByRole("button", { name: /as a golden eval/ })).not.toBeInTheDocument();
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

  it("renders canonical row slots in order and sends server-side sort choices", async () => {
    const { container } = render(<App />);
    expect(await screen.findByText(source.title)).toBeVisible();
    const row = container.querySelector(".record")!;
    expect(Array.from(row.children).slice(1).map((node) => node.className)).toEqual([
      "record-kind", "record-id", "record-main", "description-snippet", "",
    ]);
    expect(Array.from(row.querySelector(".record-main")!.children).map((node) => node.className)).toEqual(["record-title", "record-source", "record-users"]);
    fireEvent.change(screen.getByRole("combobox", { name: "Sort Objects" }), { target: { value: "connections" } });
    await waitFor(() => expect(fetch).toHaveBeenCalledWith(expect.stringContaining("sort=connections"), expect.anything()));
    await waitFor(() => expect(container.querySelector(".record-title strong")).toHaveTextContent(chat.title));
  });

  it("refreshes only resources used by the current route", async () => {
    render(<App />);
    await screen.findByText(source.title);
    vi.mocked(fetch).mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Refresh current view" }));
    await waitFor(() => expect(fetch).toHaveBeenCalledWith(expect.stringContaining("/api/v2/objects?"), expect.anything()));
    const paths = vi.mocked(fetch).mock.calls.map(([input]) => String(input));
    expect(paths.some((path) => path.includes("/api/v2/object-visuals"))).toBe(true);
    expect(paths.some((path) => /\/api\/v2\/(tasks|sources|notes|themes|runs)/.test(path))).toBe(false);
  });
});
