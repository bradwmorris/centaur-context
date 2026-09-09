import type { ConnectionGraphEdge, ConnectionGraphNode } from "./types";

export interface PositionedGraphNode extends ConnectionGraphNode {
  x: number;
  y: number;
  degree: number;
  radius: number;
  component: number;
  prominent: boolean;
}

export interface PositionedGraphEdge extends ConnectionGraphEdge {
  source: PositionedGraphNode;
  target: PositionedGraphNode;
}

export interface ConnectionGraphLayout {
  width: number;
  height: number;
  componentCount: number;
  isolatedCount: number;
  nodes: PositionedGraphNode[];
  edges: PositionedGraphEdge[];
}

interface LocalNode extends ConnectionGraphNode {
  degree: number;
  component: number;
  x: number;
  y: number;
  vx: number;
  vy: number;
}

interface LocalComponent {
  nodes: LocalNode[];
  width: number;
  height: number;
}

const GOLDEN_ANGLE = Math.PI * (3 - Math.sqrt(5));

export function layoutConnectionGraph(
  graphNodes: ConnectionGraphNode[],
  graphEdges: ConnectionGraphEdge[],
): ConnectionGraphLayout {
  const nodes = [...graphNodes].sort((left, right) => left.id.localeCompare(right.id));
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const edges = graphEdges
    .filter((edge) => nodeById.has(edge.source_object_id) && nodeById.has(edge.target_object_id))
    .sort((left, right) => left.id.localeCompare(right.id));
  const neighbours = new Map(nodes.map((node) => [node.id, new Set<string>()]));
  for (const edge of edges) {
    neighbours.get(edge.source_object_id)?.add(edge.target_object_id);
    neighbours.get(edge.target_object_id)?.add(edge.source_object_id);
  }

  const components = connectedComponents(nodes, neighbours);
  const componentById = new Map<string, number>();
  components.forEach((component, index) => component.forEach((id) => componentById.set(id, index)));
  const degrees = new Map(nodes.map((node) => [node.id, neighbours.get(node.id)?.size ?? 0]));
  const prominenceThreshold = percentileThreshold([...degrees.values()].filter(Boolean), 0.88);
  const connected = components
    .filter((component) => component.length > 1)
    .map((component) => simulateComponent(component, nodes, edges, degrees, componentById))
    .sort((left, right) => right.nodes.length - left.nodes.length || left.nodes[0].id.localeCompare(right.nodes[0].id));
  const isolated = components
    .filter((component) => component.length === 1)
    .map((component) => component[0])
    .sort();

  const positions = new Map<string, { x: number; y: number }>();
  const totalArea = connected.reduce((sum, component) => sum + component.width * component.height, 0);
  const targetWidth = Math.max(900, Math.sqrt(totalArea) * 1.35);
  const gap = 90;
  let cursorX = 60;
  let cursorY = 60;
  let rowHeight = 0;
  let occupiedWidth = 0;
  for (const component of connected) {
    if (cursorX > 60 && cursorX + component.width > targetWidth) {
      cursorX = 60;
      cursorY += rowHeight + gap;
      rowHeight = 0;
    }
    for (const node of component.nodes) {
      positions.set(node.id, { x: cursorX + node.x, y: cursorY + node.y });
    }
    cursorX += component.width + gap;
    rowHeight = Math.max(rowHeight, component.height);
    occupiedWidth = Math.max(occupiedWidth, cursorX);
  }

  if (isolated.length > 0) {
    const columns = Math.min(22, Math.max(1, Math.ceil(Math.sqrt(isolated.length * 2))));
    const isolateGap = 48;
    const isolateTop = connected.length > 0 ? cursorY + rowHeight + 110 : 80;
    isolated.forEach((id, index) => {
      positions.set(id, {
        x: 80 + (index % columns) * isolateGap,
        y: isolateTop + Math.floor(index / columns) * isolateGap,
      });
    });
    occupiedWidth = Math.max(occupiedWidth, 130 + Math.min(columns, isolated.length) * isolateGap);
    cursorY = isolateTop + Math.ceil(isolated.length / columns) * isolateGap;
  } else {
    cursorY += rowHeight;
  }

  const positionedNodes = nodes.map<PositionedGraphNode>((node) => {
    const degree = degrees.get(node.id) ?? 0;
    const position = positions.get(node.id) ?? { x: 80, y: 80 };
    return {
      ...node,
      ...position,
      degree,
      radius: 5.5 + Math.min(16, Math.log2(degree + 1) * 2.65),
      component: componentById.get(node.id) ?? 0,
      prominent: degree > 0 && degree >= prominenceThreshold,
    };
  });
  const positionedById = new Map(positionedNodes.map((node) => [node.id, node]));
  const positionedEdges = edges.map<PositionedGraphEdge>((edge) => ({
    ...edge,
    source: positionedById.get(edge.source_object_id)!,
    target: positionedById.get(edge.target_object_id)!,
  }));

  return {
    width: Math.max(900, Math.ceil(occupiedWidth + 60)),
    height: Math.max(600, Math.ceil(cursorY + 80)),
    componentCount: components.length,
    isolatedCount: isolated.length,
    nodes: positionedNodes,
    edges: positionedEdges,
  };
}

function connectedComponents(
  nodes: ConnectionGraphNode[],
  neighbours: Map<string, Set<string>>,
): string[][] {
  const remaining = new Set(nodes.map((node) => node.id));
  const components: string[][] = [];
  while (remaining.size > 0) {
    const first = [...remaining].sort()[0];
    const queue = [first];
    const component: string[] = [];
    remaining.delete(first);
    while (queue.length > 0) {
      const current = queue.shift()!;
      component.push(current);
      for (const neighbour of [...(neighbours.get(current) ?? [])].sort()) {
        if (remaining.delete(neighbour)) queue.push(neighbour);
      }
    }
    components.push(component.sort());
  }
  return components.sort((left, right) => right.length - left.length || left[0].localeCompare(right[0]));
}

function simulateComponent(
  component: string[],
  allNodes: ConnectionGraphNode[],
  allEdges: ConnectionGraphEdge[],
  degrees: Map<string, number>,
  componentById: Map<string, number>,
): LocalComponent {
  const nodeLookup = new Map(allNodes.map((node) => [node.id, node]));
  const sorted = [...component].sort((left, right) =>
    (degrees.get(right) ?? 0) - (degrees.get(left) ?? 0) || left.localeCompare(right));
  const nodes = sorted.map<LocalNode>((id, index) => {
    const radius = 28 * Math.sqrt(index);
    const angle = index * GOLDEN_ANGLE + (stableHash(id) % 360) * Math.PI / 180;
    return {
      ...nodeLookup.get(id)!,
      degree: degrees.get(id) ?? 0,
      component: componentById.get(id) ?? 0,
      x: Math.cos(angle) * radius,
      y: Math.sin(angle) * radius,
      vx: 0,
      vy: 0,
    };
  });
  const indexById = new Map(nodes.map((node, index) => [node.id, index]));
  const links = allEdges.flatMap((edge) => {
    const source = indexById.get(edge.source_object_id);
    const target = indexById.get(edge.target_object_id);
    return source === undefined || target === undefined ? [] : [[source, target] as const];
  });
  const maxDegree = Math.max(1, ...nodes.map((node) => node.degree));
  const iterations = Math.min(180, 85 + nodes.length);

  for (let tick = 0; tick < iterations; tick += 1) {
    const cooling = 1 - tick / iterations;
    for (let left = 0; left < nodes.length; left += 1) {
      for (let right = left + 1; right < nodes.length; right += 1) {
        const a = nodes[left];
        const b = nodes[right];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let distanceSquared = dx * dx + dy * dy;
        if (distanceSquared < 0.01) {
          dx = ((stableHash(`${a.id}:${b.id}`) % 17) - 8) / 10;
          dy = ((stableHash(`${b.id}:${a.id}`) % 17) - 8) / 10;
          distanceSquared = Math.max(0.01, dx * dx + dy * dy);
        }
        const force = Math.min(0.8, 540 / distanceSquared) * cooling;
        a.vx -= dx * force;
        a.vy -= dy * force;
        b.vx += dx * force;
        b.vy += dy * force;
      }
    }
    for (const [sourceIndex, targetIndex] of links) {
      const source = nodes[sourceIndex];
      const target = nodes[targetIndex];
      const dx = target.x - source.x;
      const dy = target.y - source.y;
      const distance = Math.max(1, Math.hypot(dx, dy));
      const ideal = 48 + 5 * Math.log2(source.degree + target.degree + 2);
      const force = (distance - ideal) * 0.012 * cooling;
      source.vx += dx / distance * force;
      source.vy += dy / distance * force;
      target.vx -= dx / distance * force;
      target.vy -= dy / distance * force;
    }
    for (const node of nodes) {
      const centrality = node.degree / maxDegree;
      node.vx -= node.x * (0.004 + centrality * 0.009) * cooling;
      node.vy -= node.y * (0.004 + centrality * 0.009) * cooling;
      node.vx *= 0.78;
      node.vy *= 0.78;
      node.x += clamp(node.vx, -9, 9);
      node.y += clamp(node.vy, -9, 9);
    }
  }

  for (const node of nodes) {
    const centralityScale = 1 - 0.24 * (node.degree / maxDegree);
    node.x *= centralityScale;
    node.y *= centralityScale;
  }
  const minX = Math.min(...nodes.map((node) => node.x));
  const minY = Math.min(...nodes.map((node) => node.y));
  const maxX = Math.max(...nodes.map((node) => node.x));
  const maxY = Math.max(...nodes.map((node) => node.y));
  const padding = 36;
  for (const node of nodes) {
    node.x = node.x - minX + padding;
    node.y = node.y - minY + padding;
  }
  return {
    nodes,
    width: Math.max(90, maxX - minX + padding * 2),
    height: Math.max(90, maxY - minY + padding * 2),
  };
}

function percentileThreshold(values: number[], percentile: number): number {
  if (values.length === 0) return Number.POSITIVE_INFINITY;
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * percentile))];
}

function stableHash(value: string): number {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}
