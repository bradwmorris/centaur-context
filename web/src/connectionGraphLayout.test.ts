import { describe, expect, it } from "vitest";
import { layoutConnectionGraph } from "./connectionGraphLayout";
import type { ConnectionGraphEdge, ConnectionGraphNode } from "./types";

const nodes: ConnectionGraphNode[] = [
  { id: "hub", kind: "entity", title: "Hub" },
  { id: "leaf-a", kind: "source", title: "Leaf A" },
  { id: "leaf-b", kind: "note", title: "Leaf B" },
  { id: "leaf-c", kind: "memory", title: "Leaf C" },
  { id: "pair-a", kind: "user", title: "Pair A" },
  { id: "pair-b", kind: "chat", title: "Pair B" },
  { id: "isolated", kind: "task", title: "Isolated" },
];

const edges: ConnectionGraphEdge[] = [
  { id: "edge-a", source_object_id: "hub", target_object_id: "leaf-a", kind: "about", description: "Hub points to A." },
  { id: "edge-b", source_object_id: "hub", target_object_id: "leaf-b", kind: "related_to", description: "Hub points to B." },
  { id: "edge-c", source_object_id: "leaf-c", target_object_id: "hub", kind: "derived_from", description: "C points to hub." },
  { id: "edge-pair", source_object_id: "pair-a", target_object_id: "pair-b", kind: "involves", description: "Pair A involves Pair B." },
];

describe("connection graph layout", () => {
  it("is deterministic and preserves directed edge identities", () => {
    const first = layoutConnectionGraph(nodes, edges);
    const second = layoutConnectionGraph([...nodes].reverse(), [...edges].reverse());
    expect(second).toEqual(first);
    expect(first.edges.map((edge) => [edge.id, edge.source.id, edge.target.id])).toContainEqual(["edge-c", "leaf-c", "hub"]);
    expect(first.componentCount).toBe(3);
    expect(first.isolatedCount).toBe(1);
  });

  it("promotes the highest-degree Object toward its cluster centre", () => {
    const layout = layoutConnectionGraph(nodes, edges);
    const cluster = layout.nodes.filter((node) => ["hub", "leaf-a", "leaf-b", "leaf-c"].includes(node.id));
    const hub = cluster.find((node) => node.id === "hub")!;
    const centroid = {
      x: cluster.reduce((sum, node) => sum + node.x, 0) / cluster.length,
      y: cluster.reduce((sum, node) => sum + node.y, 0) / cluster.length,
    };
    const distance = (node: typeof hub) => Math.hypot(node.x - centroid.x, node.y - centroid.y);
    const leafAverage = cluster.filter((node) => node.id !== "hub").reduce((sum, node) => sum + distance(node), 0) / 3;
    expect(hub.degree).toBe(3);
    expect(hub.radius).toBeGreaterThan(cluster.find((node) => node.id === "leaf-a")!.radius);
    expect(distance(hub)).toBeLessThan(leafAverage);
    expect(hub.prominent).toBe(true);
  });

  it("keeps isolated Objects and safely ignores incomplete edges", () => {
    const layout = layoutConnectionGraph(nodes, [...edges, { id: "broken", source_object_id: "hub", target_object_id: "missing", kind: "about", description: "Invalid endpoint." }]);
    expect(layout.nodes.find((node) => node.id === "isolated")?.degree).toBe(0);
    expect(layout.edges.some((edge) => edge.id === "broken")).toBe(false);
    expect(layout.width).toBeGreaterThanOrEqual(900);
    expect(layout.height).toBeGreaterThanOrEqual(600);
  });

  it("handles empty and single-node graphs", () => {
    expect(layoutConnectionGraph([], [])).toMatchObject({ componentCount: 0, isolatedCount: 0, nodes: [], edges: [] });
    const single = layoutConnectionGraph([{ id: "only", kind: "memory", title: "Only" }], []);
    expect(single).toMatchObject({ componentCount: 1, isolatedCount: 1 });
    expect(single.nodes[0]).toMatchObject({ id: "only", degree: 0 });
  });
});
