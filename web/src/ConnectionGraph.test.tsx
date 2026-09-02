import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ConnectionGraphWorkspace, FocusedObjectGraph } from "./ConnectionGraph";
import type { ConnectionGraphSnapshot } from "./types";

const graph: ConnectionGraphSnapshot = {
  fingerprint: "fingerprint123",
  node_count: 4,
  connection_count: 2,
  nodes: [
    { id: "hub", kind: "entity", title: "Central Object" },
    { id: "leaf-a", kind: "source", title: "Source Leaf" },
    { id: "leaf-b", kind: "note", title: "Note Leaf" },
    { id: "isolated", kind: "memory", title: "Quiet Object" },
  ],
  edges: [
    { id: "edge-a", source_object_id: "hub", target_object_id: "leaf-a", kind: "about", description: "Central Object is about Source Leaf." },
    { id: "edge-b", source_object_id: "leaf-b", target_object_id: "hub", kind: "derived_from", description: "Note Leaf is derived from Central Object." },
  ],
};

function mockGraph(snapshot: ConnectionGraphSnapshot = graph) {
  vi.stubGlobal("fetch", vi.fn(() => Promise.resolve(new Response(JSON.stringify({ data: snapshot }), { status: 200, headers: { "Content-Type": "application/json" } }))));
}

afterEach(() => {
  vi.unstubAllGlobals();
  window.history.replaceState({}, "", "/");
});

describe("Connections graph workspace", () => {
  it("loads one compact graph and focuses searched Objects", async () => {
    mockGraph();
    render(<ConnectionGraphWorkspace />);
    expect(await screen.findByRole("img", { name: /4 Objects, 2 Connections, and 2 connected clusters/ })).toBeVisible();
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fetch).toHaveBeenCalledWith("/api/v2/connection-graph", expect.anything());
    fireEvent.change(screen.getByRole("textbox", { name: "Search graph Objects" }), { target: { value: "Central" } });
    fireEvent.click(screen.getByRole("button", { name: "Focus" }));
    expect(await screen.findByRole("heading", { name: "Central Object" })).toBeVisible();
    expect(screen.getByRole("link", { name: "Open Object detail" })).toHaveAttribute("href", "/objects/hub");
    expect(screen.getByText("2 direct Connections · cluster 1")).toBeVisible();
  });

  it("opens directed Connection context from a focused neighbourhood", async () => {
    mockGraph();
    render(<ConnectionGraphWorkspace />);
    fireEvent.click(await screen.findByRole("button", { name: /Central Object, entity Object/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Central Object about Source Leaf/ }));
    expect(screen.getByRole("heading", { name: "about" })).toBeVisible();
    expect(screen.getByText("Central Object is about Source Leaf.")).toBeVisible();
    expect(screen.getByRole("link", { name: "Open Connection detail" })).toHaveAttribute("href", "/connections/edge-a");
  });

  it("renders one Object neighbourhood and links to the full graph with focus", async () => {
    mockGraph();
    render(<FocusedObjectGraph objectId="hub" objectTitle="Central Object" />);
    const focusedGraph = await screen.findByRole("img", { name: "Central Object with 2 related Objects and 2 direct Connections" });
    expect(focusedGraph).toBeVisible();
    const fittedViewBox = focusedGraph.getAttribute("viewBox");
    expect(fittedViewBox).not.toBe("0 0 900 600");
    fireEvent.click(screen.getByRole("button", { name: "Zoom in focused graph" }));
    expect(focusedGraph.getAttribute("viewBox")).not.toBe(fittedViewBox);
    fireEvent.click(screen.getByRole("button", { name: "Fit" }));
    expect(focusedGraph.getAttribute("viewBox")).not.toBe(fittedViewBox);
    expect(screen.getByRole("link", { name: "Open in Connections" })).toHaveAttribute("href", "/connections?object=hub");
    expect(screen.getByRole("button", { name: /Central Object, entity Object/ })).toBeVisible();
    expect(screen.queryByRole("button", { name: /Quiet Object/ })).not.toBeInTheDocument();
  });

  it("opens the full graph focused on the Object from its query link", async () => {
    window.history.replaceState({}, "", "/connections?object=hub");
    mockGraph();
    render(<ConnectionGraphWorkspace />);
    expect(await screen.findByLabelText("Selected Object")).toBeVisible();
    expect(screen.getByRole("heading", { name: "Central Object" })).toBeVisible();
  });

  it("supports fit, zoom, Escape, and empty states", async () => {
    mockGraph();
    const { unmount } = render(<ConnectionGraphWorkspace />);
    const canvas = await screen.findByRole("img");
    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    fireEvent.click(screen.getByRole("button", { name: "Fit" }));
    fireEvent.click(screen.getByRole("button", { name: /Central Object, entity Object/ }));
    fireEvent.keyDown(canvas, { key: "Escape" });
    await waitFor(() => expect(screen.queryByLabelText("Selected Object")).not.toBeInTheDocument());
    expect(screen.queryByLabelText("Graph summary")).not.toBeInTheDocument();
    unmount();

    mockGraph({ ...graph, node_count: 0, connection_count: 0, nodes: [], edges: [] });
    render(<ConnectionGraphWorkspace />);
    expect(await screen.findByText("No active Objects yet.")).toBeVisible();
  });
});
