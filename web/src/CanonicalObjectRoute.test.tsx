import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { webcrypto } from "node:crypto";
import App from "./App";
import { CanonicalObjectRoute } from "./CanonicalObjectRoute";
import { interceptNavigation, navigate, objectDetailPath } from "./routing";
import fixture from "./test-fixtures/object-routes.json";
import { createObjectFixtureApi } from "./test-fixtures/objectApi";

beforeEach(() => { vi.stubGlobal("crypto", webcrypto); vi.stubGlobal("fetch", vi.fn(createObjectFixtureApi({ emptyList: true }))); });
const sections = { event: "events", task: "tasks", chat: "chats", user: "users", entity: "entities", memory: "memories", source: "sources", note: "notes", theme: "themes" };
const content: Record<string, string> = { note: fixture.subtypes.note.content, task: fixture.subtypes.task.brief_markdown, memory: fixture.subtypes.memory.happened_at, chat: "Synthetic chat transcript message.", source: "Synthetic author", theme: "Themed Objects", user: "Human", entity: "Organization", event: "Occurrence" };

describe("complete Object detail routing", () => {
  for (const [kind, section] of Object.entries(sections)) {
    it.each(["objects", section, section === "sources" ? "notes" : "sources"])(`${kind} resolves from /%s without a loaded collection`, async alias => {
      window.history.replaceState({}, "", `/${alias}/synthetic-${kind}?ref=fixture#details`);
      render(<App />);
      expect(await screen.findByText(content[kind].replace(/\s+/g, " "))).toBeInTheDocument();
      expect(window.location.pathname).toBe(`/${section}/synthetic-${kind}`);
      expect(screen.queryByRole("button", { name: "Review or correct evidence" })).not.toBeInTheDocument();
      expect(window.location.search + window.location.hash).toBe("?ref=fixture#details");
      expect(screen.getByRole("button", { name: `Edit ${["note", "task", "source", "theme"].includes(kind) ? kind[0].toUpperCase() + kind.slice(1) : "Object"} description` })).toBeInTheDocument();
    });
  }
  it("opens a searched Note row and Related Object as a complete Note", async () => {
    vi.stubGlobal("fetch", vi.fn(createObjectFixtureApi()));
    window.history.replaceState({}, "", "/objects");
    render(<App />);
    fireEvent.change(screen.getByRole("textbox", { name: "Search objects" }), { target: { value: "Synthetic note" } });
    await waitFor(() => expect(screen.queryByRole("link", { name: "Open Synthetic task" })).not.toBeInTheDocument());
    fireEvent.click(await screen.findByRole("link", { name: "Open Synthetic note" }));
    expect(await screen.findByText(fixture.subtypes.note.content)).toBeVisible();
    act(() => navigate("/sources/synthetic-source"));
    fireEvent.click(await screen.findByRole("link", { name: "Synthetic note" }));
    expect(await screen.findByText(fixture.subtypes.note.content)).toBeVisible();
    expect(window.location.pathname).toBe("/notes/synthetic-note");
  });
  it("opens a Task from a focused graph and retains workflow fields", async () => {
    window.history.replaceState({}, "", "/notes/synthetic-note");
    render(<App />);
    fireEvent.click(await screen.findByText("Connection map", { selector: "summary" }));
    fireEvent.click(await screen.findByRole("link", { name: /Synthetic task, task Object/ }));
    expect(await screen.findByRole("combobox", { name: "Task status" })).toHaveValue("todo");
    expect(screen.getByRole("button", { name: "Edit Task brief" })).toHaveTextContent("Prepare the synthetic review.");
    expect(window.location.pathname).toBe("/tasks/synthetic-task");
  });
  it("retains revision-aware shared description editing on the complete Note", async () => {
    window.history.replaceState({}, "", "/objects/synthetic-note");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Edit Note description" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Note description" }), { target: { value: "Updated synthetic description." } });
    fireEvent.click(screen.getByRole("button", { name: "Save Note description" }));
    await waitFor(() => expect(fetch).toHaveBeenCalledWith("/api/v2/objects/synthetic-note", expect.objectContaining({ method: "PATCH", body: JSON.stringify({ expected_revision: 1, description: "Updated synthetic description." }) })));
    fireEvent.click(screen.getByRole("button", { name: "Edit Note content" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Note content" }), { target: { value: "Updated body." } });
    fireEvent.click(screen.getByRole("button", { name: "Save Note content" }));
    await waitFor(() => expect(fetch).toHaveBeenCalledWith("/api/v2/notes/synthetic-note", expect.objectContaining({ method: "PATCH", body: expect.stringContaining('"expected_revision":2') })));
  });
  it.each(["artifacts/synthetic-note-artifact-1", "working/research"])("preserves the nested %s destination", async suffix => {
    window.history.replaceState({}, "", `/tasks/synthetic-note/${suffix}?view=read#body`);
    const historyLength = window.history.length;
    render(<App />);
    expect(await screen.findByText(fixture.content)).toBeVisible();
    expect(window.location.pathname).toBe(`/notes/synthetic-note/${suffix}`);
    expect(window.location.search + window.location.hash).toBe("?view=read#body");
    expect(window.history.length).toBe(historyLength);
    fireEvent.click(screen.getByRole("link", { name: "← Back to Object" }));
    expect(await screen.findByText(fixture.subtypes.note.content)).toBeVisible();
  });
  it("exposes existing Entity kind and image without introducing editing", async () => {
    window.history.replaceState({}, "", "/objects/synthetic-entity");
    render(<App />);
    fireEvent.click(await screen.findByText("Entity details"));
    expect(await screen.findByText("Kind: organization")).toBeVisible();
    expect(screen.getByAltText("Entity image")).toHaveAttribute("src", fixture.subtypes.entity.image_url);
    expect(screen.queryByRole("button", { name: "Save properties" })).not.toBeInTheDocument();
  });
  it("keeps an unknown future kind on a safe generic page", async () => {
    const fallback = createObjectFixtureApi();
    vi.stubGlobal("fetch", vi.fn((input, init) => String(input) === "/api/v2/objects/synthetic-entity" ? Promise.resolve(new Response(JSON.stringify({ data: { ...fixture.objects.find(o => o.kind === "entity"), kind: "future_kind" } }))) : fallback(input, init)));
    window.history.replaceState({}, "", "/entities/synthetic-entity");
    render(<App />);
    expect(await screen.findByRole("button", { name: "Edit Object description" })).toBeVisible();
    expect(window.location.pathname).toBe("/objects/synthetic-entity");
    expect(screen.getByText("future_kind")).toBeVisible();
  });
  it.each([403, 404, 500])("shows an honest resolver error for HTTP %s", async status => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ error: { message: "Synthetic read failed" } }), { status })));
    window.history.replaceState({}, "", "/objects/missing");
    render(<CanonicalObjectRoute id="missing" artifact={null}><p>Complete detail</p></CanonicalObjectRoute>);
    expect(await screen.findByText("Synthetic read failed")).toBeVisible();
    expect(screen.queryByText("Complete detail")).not.toBeInTheDocument();
  });
  it("retains native modifier-click and respects unsaved navigation cancellation", () => {
    window.history.replaceState({}, "", "/notes/synthetic-note");
    const event = { button: 0, metaKey: true, preventDefault: vi.fn() };
    interceptNavigation(event as unknown as React.MouseEvent<HTMLAnchorElement>, "/tasks/synthetic-task");
    expect(event.preventDefault).not.toHaveBeenCalled();
    const stop = (e: Event) => e.preventDefault();
    window.addEventListener("beforeappnavigate", stop);
    navigate("/tasks/synthetic-task");
    expect(window.location.pathname).toBe("/notes/synthetic-note");
    window.removeEventListener("beforeappnavigate", stop);
    expect(objectDetailPath("unknown", "id")).toBe("/objects/id");
  });
});
