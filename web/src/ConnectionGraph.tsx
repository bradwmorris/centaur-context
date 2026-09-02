import { FormEvent, PointerEvent, WheelEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import { layoutConnectionGraph } from "./connectionGraphLayout";
import type { PositionedGraphEdge, PositionedGraphNode } from "./connectionGraphLayout";
import { ObjectTypeBadge } from "./RecordVisuals";
import { connectionPath, interceptNavigation, objectPath } from "./routing";
import type { ConnectionGraphSnapshot } from "./types";

type Selection = { type: "node" | "edge"; id: string } | null;
interface Transform { x: number; y: number; scale: number }
interface DragState { pointerId: number; clientX: number; clientY: number; x: number; y: number }

const initialTransform: Transform = { x: 0, y: 0, scale: 1 };

export function ConnectionGraphWorkspace({ refreshKey = 0 }: { refreshKey?: number }) {
  const [graph, setGraph] = useState<ConnectionGraphSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [selection, setSelection] = useState<Selection>(null);
  const [hoveredNode, setHoveredNode] = useState<string | null>(null);
  const [transform, setTransform] = useState<Transform>(initialTransform);
  const drag = useRef<DragState | null>(null);
  const loadGeneration = useRef(0);

  const load = useCallback(() => {
    const generation = ++loadGeneration.current;
    setLoading(true);
    setError(null);
    void api.connectionGraph()
      .then((snapshot) => {
        if (generation !== loadGeneration.current) return;
        setGraph(snapshot);
        setSelection((current) => selectionExists(current, snapshot) ? current : null);
      })
      .catch((cause) => { if (generation === loadGeneration.current) setError(cause instanceof Error ? cause.message : "The graph could not be loaded."); })
      .finally(() => { if (generation === loadGeneration.current) setLoading(false); });
  }, []);
  useEffect(() => load(), [load, refreshKey]);

  const layout = useMemo(
    () => layoutConnectionGraph(graph?.nodes ?? [], graph?.edges ?? []),
    [graph],
  );
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const matches = useMemo(() => new Set(layout.nodes
    .filter((node) => normalizedQuery && `${node.title} ${node.kind} ${node.id}`.toLocaleLowerCase().includes(normalizedQuery))
    .map((node) => node.id)), [layout.nodes, normalizedQuery]);
  const selectedNode = selection?.type === "node" ? layout.nodes.find((node) => node.id === selection.id) ?? null : null;
  const selectedEdge = selection?.type === "edge" ? layout.edges.find((edge) => edge.id === selection.id) ?? null : null;
  const incidentIds = useMemo(() => new Set(selectedNode ? layout.edges
    .filter((edge) => edge.source.id === selectedNode.id || edge.target.id === selectedNode.id)
    .flatMap((edge) => [edge.id, edge.source.id, edge.target.id]) : []), [layout.edges, selectedNode]);
  const submitSearch = (event: FormEvent) => {
    event.preventDefault();
    const match = [...layout.nodes]
      .filter((node) => matches.has(node.id))
      .sort((left, right) => searchRank(left, normalizedQuery) - searchRank(right, normalizedQuery) || right.degree - left.degree || left.title.localeCompare(right.title))[0];
    if (match) setSelection({ type: "node", id: match.id });
  };
  const zoom = (factor: number) => setTransform((current) => {
    const scale = clamp(current.scale * factor, 0.35, 3.2);
    const ratio = scale / current.scale;
    return {
      scale,
      x: layout.width / 2 - (layout.width / 2 - current.x) * ratio,
      y: layout.height / 2 - (layout.height / 2 - current.y) * ratio,
    };
  });
  const onWheel = (event: WheelEvent<SVGSVGElement>) => {
    event.preventDefault();
    zoom(event.deltaY > 0 ? 0.9 : 1.1);
  };
  const startPan = (event: PointerEvent<SVGSVGElement>) => {
    if (event.target !== event.currentTarget) return;
    drag.current = { pointerId: event.pointerId, clientX: event.clientX, clientY: event.clientY, x: transform.x, y: transform.y };
    event.currentTarget.setPointerCapture(event.pointerId);
    setSelection(null);
  };
  const movePan = (event: PointerEvent<SVGSVGElement>) => {
    if (!drag.current || drag.current.pointerId !== event.pointerId) return;
    const scaleX = layout.width / Math.max(1, event.currentTarget.clientWidth);
    const scaleY = layout.height / Math.max(1, event.currentTarget.clientHeight);
    setTransform((current) => ({
      ...current,
      x: drag.current!.x + (event.clientX - drag.current!.clientX) * scaleX,
      y: drag.current!.y + (event.clientY - drag.current!.clientY) * scaleY,
    }));
  };
  const endPan = (event: PointerEvent<SVGSVGElement>) => {
    if (drag.current?.pointerId === event.pointerId) drag.current = null;
  };

  if (loading && !graph) return <div className="connection-graph-blank">Mapping Objects and Connections…</div>;
  if (error && !graph) return <div className="connection-graph-blank error"><p>{error}</p><button className="secondary" onClick={load}>Try again</button></div>;

  return <section className="connection-graph-workspace" aria-label="Connections graph">
    <header className="connection-graph-toolbar">
      <div>
        <h1>Connections</h1>
        <p>{graph?.node_count ?? 0} Objects · {graph?.connection_count ?? 0} Connections · {layout.componentCount} clusters</p>
      </div>
      <form className="connection-graph-search" onSubmit={submitSearch} role="search">
        <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.25" /><path d="m10.25 10.25 3 3" /></svg>
        <input aria-label="Search graph Objects" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Find an Object" />
        {normalizedQuery && <span aria-live="polite">{matches.size}</span>}
        {normalizedQuery && <button type="submit">Focus</button>}
      </form>
      <div className="connection-graph-controls" aria-label="Graph controls">
        <button type="button" onClick={() => zoom(0.82)} aria-label="Zoom out">−</button>
        <button type="button" onClick={() => setTransform(initialTransform)}>Fit</button>
        <button type="button" onClick={() => zoom(1.22)} aria-label="Zoom in">+</button>
        <button type="button" onClick={load} aria-label="Refresh graph">↻</button>
      </div>
    </header>

    {error && <div className="connection-graph-notice">{error}<button onClick={load}>Retry</button></div>}
    {layout.nodes.length === 0 ? <div className="connection-graph-blank">No active Objects yet.</div> : <div className="connection-graph-body">
      <div className="connection-graph-canvas">
        <svg
          viewBox={`0 0 ${layout.width} ${layout.height}`}
          role="img"
          aria-label={`Connection map with ${layout.nodes.length} Objects, ${layout.edges.length} Connections, and ${layout.componentCount} connected clusters.`}
          onWheel={onWheel}
          onPointerDown={startPan}
          onPointerMove={movePan}
          onPointerUp={endPan}
          onPointerCancel={endPan}
          onKeyDown={(event) => { if (event.key === "Escape") setSelection(null); }}
        >
          <defs>
            <marker id="connection-arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="4" markerHeight="4" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" /></marker>
          </defs>
          <g transform={`translate(${transform.x} ${transform.y}) scale(${transform.scale})`}>
            <g className="connection-graph-edges">
              {layout.edges.map((edge) => <GraphEdge
                key={edge.id}
                edge={edge}
                active={!selectedNode || incidentIds.has(edge.id)}
                selected={selectedEdge?.id === edge.id}
                keyboardEnabled={Boolean(selectedNode && incidentIds.has(edge.id))}
                onSelect={() => setSelection({ type: "edge", id: edge.id })}
              />)}
            </g>
            <g className="connection-graph-nodes">
              {layout.nodes.map((node) => <GraphNode
                key={node.id}
                node={node}
                matched={!normalizedQuery || matches.has(node.id)}
                active={!selectedNode || incidentIds.has(node.id)}
                selected={selectedNode?.id === node.id}
                hovered={hoveredNode === node.id}
                onHover={setHoveredNode}
                onSelect={() => setSelection({ type: "node", id: node.id })}
              />)}
            </g>
          </g>
        </svg>
        <p className="connection-graph-hint">Scroll to zoom · drag empty space to pan · Escape clears focus</p>
      </div>
      <GraphInspector
        node={selectedNode}
        edge={selectedEdge}
        onSelectNode={(id) => setSelection({ type: "node", id })}
      />
    </div>}
    <MobileGraphList nodes={layout.nodes} onSelect={(id) => setSelection({ type: "node", id })} />
    <span className="connection-graph-fingerprint">Snapshot {graph?.fingerprint.slice(0, 12)}</span>
  </section>;
}

function GraphNode({ node, matched, active, selected, hovered, onHover, onSelect }: {
  node: PositionedGraphNode;
  matched: boolean;
  active: boolean;
  selected: boolean;
  hovered: boolean;
  onHover: (id: string | null) => void;
  onSelect: () => void;
}) {
  const showLabel = node.prominent || selected || hovered;
  const className = ["connection-graph-node", `kind-${node.kind}`, node.prominent ? "prominent" : "", !matched || !active ? "muted" : "", selected ? "selected" : ""].filter(Boolean).join(" ");
  const activate = (event: React.KeyboardEvent<SVGGElement>) => {
    if (event.key === "Enter" || event.key === " ") { event.preventDefault(); onSelect(); }
  };
  return <g className={className} transform={`translate(${node.x} ${node.y})`} role="button" tabIndex={0} aria-label={`${node.title}, ${node.kind} Object, ${node.degree} direct Connections`} onClick={(event) => { event.stopPropagation(); onSelect(); }} onKeyDown={activate} onMouseEnter={() => onHover(node.id)} onMouseLeave={() => onHover(null)}>
    <circle r={node.radius + 6} className="node-hit-area" />
    <circle r={node.radius} className="node-dot" />
    {showLabel && <text x={node.radius + 7} y="4" className="node-label">{truncate(node.title, 42)}</text>}
  </g>;
}

function GraphEdge({ edge, active, selected, keyboardEnabled, onSelect }: {
  edge: PositionedGraphEdge;
  active: boolean;
  selected: boolean;
  keyboardEnabled: boolean;
  onSelect: () => void;
}) {
  const className = ["connection-graph-edge", !active ? "muted" : "", selected ? "selected" : ""].filter(Boolean).join(" ");
  const path = `M ${edge.source.x} ${edge.source.y} L ${edge.target.x} ${edge.target.y}`;
  const activate = (event: React.KeyboardEvent<SVGPathElement>) => {
    if (event.key === "Enter" || event.key === " ") { event.preventDefault(); onSelect(); }
  };
  return <g className={className}>
    <path d={path} className="edge-line" markerEnd="url(#connection-arrow)" />
    <path d={path} className="edge-hit-area" role={keyboardEnabled ? "button" : undefined} tabIndex={keyboardEnabled ? 0 : -1} aria-label={keyboardEnabled ? `${edge.source.title} ${edge.kind.replaceAll("_", " ")} ${edge.target.title}: ${edge.description}` : undefined} onClick={(event) => { event.stopPropagation(); onSelect(); }} onKeyDown={activate} />
  </g>;
}

function GraphInspector({ node, edge, onSelectNode }: {
  node: PositionedGraphNode | null;
  edge: PositionedGraphEdge | null;
  onSelectNode: (id: string) => void;
}) {
  if (edge) return <aside className="connection-graph-inspector" aria-label="Selected Connection">
    <span className="inspector-eyebrow">Connection</span>
    <h2>{edge.kind.replaceAll("_", " ")}</h2>
    <p>{edge.description}</p>
    <div className="inspector-flow">
      <button onClick={() => onSelectNode(edge.source.id)}>{edge.source.title}</button><span>→</span><button onClick={() => onSelectNode(edge.target.id)}>{edge.target.title}</button>
    </div>
    <a href={connectionPath(edge.id)} onClick={(event) => interceptNavigation(event, connectionPath(edge.id))}>Open Connection detail</a>
  </aside>;
  if (node) return <aside className="connection-graph-inspector" aria-label="Selected Object">
      <span className="inspector-eyebrow"><ObjectTypeBadge kind={node.kind} /></span>
      <h2>{node.title}</h2>
      <p>{node.degree} direct {node.degree === 1 ? "Connection" : "Connections"} · cluster {node.component + 1}</p>
      <a href={objectPath(node.id)} onClick={(event) => interceptNavigation(event, objectPath(node.id))}>Open Object detail</a>
    </aside>;
  return null;
}

function MobileGraphList({ nodes, onSelect }: { nodes: PositionedGraphNode[]; onSelect: (id: string) => void }) {
  const ordered = [...nodes].sort((left, right) => left.component - right.component || right.degree - left.degree || left.title.localeCompare(right.title));
  return <section className="connection-graph-mobile-list" aria-label="Objects ordered by graph cluster">
    <h2>Objects by cluster</h2>
    <ol>{ordered.map((node) => <li key={node.id}><button onClick={() => onSelect(node.id)}><span>{node.title}<small>{node.kind} · cluster {node.component + 1}</small></span><b>{node.degree}</b></button></li>)}</ol>
  </section>;
}

function selectionExists(selection: Selection, graph: ConnectionGraphSnapshot): boolean {
  if (!selection) return false;
  return selection.type === "node"
    ? graph.nodes.some((node) => node.id === selection.id)
    : graph.edges.some((edge) => edge.id === selection.id);
}

function truncate(value: string, limit: number): string {
  return value.length <= limit ? value : `${value.slice(0, limit - 1)}…`;
}

function searchRank(node: PositionedGraphNode, query: string): number {
  const title = node.title.toLocaleLowerCase();
  if (title === query) return 0;
  if (title.startsWith(query)) return 1;
  return 2;
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}
