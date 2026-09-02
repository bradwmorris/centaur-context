import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import { interceptNavigation, objectPath, navigate, schemaPath, schemaRowPath, schemaView } from "./routing";
import type { SchemaForeignKey, SchemaRowPage, SchemaSnapshot, SchemaTable } from "./types";

interface Props {
  selectedTable: string | null;
  refreshKey?: number;
}

const countFormatter = new Intl.NumberFormat(undefined, { notation: "compact", maximumFractionDigits: 1 });

export function SchemaWorkspace({ selectedTable, refreshKey = 0 }: Props) {
  const [snapshot, setSnapshot] = useState<SchemaSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const loadGeneration = useRef(0);
  const mode = schemaView(window.location.pathname);

  const refresh = useCallback(async () => {
    const generation = ++loadGeneration.current;
    try {
      const next = await api.schema();
      if (generation !== loadGeneration.current) return;
      setSnapshot(next);
      setError(null);
      if (selectedTable && !next.tables.some((table) => table.name === selectedTable)) {
        navigate(schemaPath());
      }
    } catch (cause) {
      if (generation === loadGeneration.current) setError(cause instanceof Error ? cause.message : "The schema could not be loaded.");
    } finally {
      if (generation === loadGeneration.current) setLoading(false);
    }
  }, [selectedTable]);

  useEffect(() => {
    void refresh();
  }, [refresh, refreshKey]);

  const table = snapshot?.tables.find((item) => item.name === selectedTable) ?? null;
  return <section className="schema-workspace" aria-label="Database schema">
    <div className="schema-main">
      {mode === "rows" && table && <header className="schema-toolbar">
        <a className="schema-back-link" href={schemaPath()} onClick={(event) => interceptNavigation(event, schemaPath())}>← Schema map</a>
        <div><h1>{tableLabel(table.name)}</h1><p>{table.classification} table · {table.columns.length} columns</p></div>
      </header>}
      {error && <div className="schema-message error">{error}<button onClick={() => setError(null)} aria-label="Dismiss error">×</button></div>}
      {loading && !snapshot ? <div className="schema-blank">Reading the live schema…</div> : !snapshot ? <div className="schema-blank">Schema unavailable.</div> : mode === "map" ? <SchemaMap snapshot={snapshot} /> : !table ? <div className="schema-blank">Choose a table to inspect.</div> : <TableRows table={table} fingerprint={snapshot.fingerprint} foreignKeys={snapshot.foreign_keys} />}
    </div>
  </section>;
}

interface NodePosition { x: number; y: number; }

function SchemaMap({ snapshot }: { snapshot: SchemaSnapshot }) {
  const layout = useMemo(() => schemaLayout(snapshot), [snapshot]);
  const [focusedTable, setFocusedTable] = useState<string | null>(null);
  const neighbourhood = useMemo(() => {
    if (!focusedTable) return null;
    const related = new Set([focusedTable]);
    snapshot.foreign_keys.forEach((edge) => {
      if (edge.source_table === focusedTable) related.add(edge.target_table);
      if (edge.target_table === focusedTable) related.add(edge.source_table);
    });
    return related;
  }, [focusedTable, snapshot.foreign_keys]);
  return <div className="schema-map-wrap">
    <div className="schema-map" style={{ width: layout.width, height: layout.height }} aria-label={`${snapshot.tables.length} tables and ${snapshot.foreign_keys.length} relationships`}>
      <svg aria-hidden="true" width={layout.width} height={layout.height} viewBox={`0 0 ${layout.width} ${layout.height}`}>
        {snapshot.foreign_keys.map((edge) => {
          const source = layout.positions.get(edge.source_table);
          const target = layout.positions.get(edge.target_table);
          if (!source || !target) return null;
          const fromX = source.x > target.x ? source.x : source.x + 190;
          const toX = source.x > target.x ? target.x + 190 : target.x;
          const fromY = source.y + 31;
          const toY = target.y + 31;
          const middle = (fromX + toX) / 2;
          const muted = focusedTable && edge.source_table !== focusedTable && edge.target_table !== focusedTable;
          return <path key={`${edge.source_table}:${edge.name}`} className={`schema-edge${edge.one_to_one_subtype ? " subtype" : ""}${muted ? " muted" : ""}`} d={`M ${fromX} ${fromY} H ${middle} V ${toY} H ${toX}`} />;
        })}
      </svg>
      {snapshot.tables.map((table) => {
        const position = layout.positions.get(table.name)!;
        const muted = neighbourhood && !neighbourhood.has(table.name);
        return <button key={table.name} className={`schema-node ${table.classification}${muted ? " muted" : ""}`} style={{ left: position.x, top: position.y }} onFocus={() => setFocusedTable(table.name)} onBlur={() => setFocusedTable(null)} onMouseEnter={() => setFocusedTable(table.name)} onMouseLeave={() => setFocusedTable(null)} onClick={() => navigate(schemaPath(table.name))}>
          <span><strong>{tableLabel(table.name)}</strong><small>{table.classification}</small></span>
          <span><b>{table.columns.length}</b> cols<small>≈ {formatCount(table.estimated_row_count)} rows</small></span>
        </button>;
      })}
    </div>
  </div>;
}

function schemaLayout(snapshot: SchemaSnapshot) {
  if (snapshot.tables.length === 0) {
    return { positions: new Map<string, NodePosition>(), width: 760, height: 460 };
  }
  const adjacency = new Map<string, Set<string>>(snapshot.tables.map((table) => [table.name, new Set()]));
  snapshot.foreign_keys.forEach((edge) => {
    adjacency.get(edge.source_table)?.add(edge.target_table);
    adjacency.get(edge.target_table)?.add(edge.source_table);
  });
  const level = new Map<string, number>([["objects", 0]]);
  const queue = ["objects"];
  while (queue.length) {
    const current = queue.shift()!;
    for (const adjacent of adjacency.get(current) ?? []) {
      if (!level.has(adjacent)) {
        level.set(adjacent, (level.get(current) ?? 0) + 1);
        queue.push(adjacent);
      }
    }
  }
  const connectedMax = Math.max(0, ...level.values());
  snapshot.tables.forEach((table) => {
    if (table.classification === "subtype") level.set(table.name, 1);
    if (!level.has(table.name)) level.set(table.name, connectedMax + 1);
  });
  const groups = new Map<number, SchemaTable[]>();
  snapshot.tables.forEach((table) => {
    const tableLevel = level.get(table.name) ?? 0;
    groups.set(tableLevel, [...(groups.get(tableLevel) ?? []), table]);
  });
  groups.forEach((tables) => tables.sort((a, b) => a.name.localeCompare(b.name)));
  const positions = new Map<string, NodePosition>();
  let maxRows = 1;
  [...groups.entries()].sort(([a], [b]) => a - b).forEach(([tableLevel, tables]) => {
    maxRows = Math.max(maxRows, tables.length);
    tables.forEach((table, index) => positions.set(table.name, { x: 30 + tableLevel * 270, y: 26 + index * 88 }));
  });
  const maximumLevel = Math.max(0, ...groups.keys());
  return { positions, width: Math.max(760, (maximumLevel + 1) * 270 + 20), height: Math.max(460, maxRows * 88 + 30) };
}

function TableRows({ table, fingerprint, foreignKeys }: { table: SchemaTable; fingerprint: string; foreignKeys: SchemaForeignKey[] }) {
  const [page, setPage] = useState<SchemaRowPage | null>(null);
  const [rows, setRows] = useState<Array<Record<string, string | null>>>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const routeSearch = window.location.search;
  const focus = useMemo(() => {
    const params = new URLSearchParams(routeSearch);
    const column = params.get("focus_column");
    const value = params.get("focus_value");
    return column !== null && value !== null ? { column, value } : undefined;
  }, [routeSearch, table.name]);
  const load = useCallback(async (cursor?: string, append = false) => {
    setLoading(true);
    try {
      const next = await api.schemaRows(table.name, cursor, cursor ? undefined : focus);
      setPage(next);
      setRows((current) => append ? [...current, ...next.rows] : next.rows);
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Rows could not be loaded.");
    } finally {
      setLoading(false);
    }
  }, [focus, table.name]);
  useEffect(() => { setRows([]); void load(); }, [fingerprint, load]);

  return <div className="schema-rows-view">
    {error && <div className="schema-message error">{error}</div>}
    <div className="schema-grid-scroll">
      <table className="schema-grid">
        <thead><tr>{table.columns.map((column) => <th key={column.name}><span>{column.name}</span><small>{column.data_type}</small></th>)}</tr></thead>
        <tbody>{rows.map((row, index) => <tr key={rowKey(row, table, index)}>{table.columns.map((column) => <td key={column.name}><CellValue value={row[column.name] ?? null} column={column.name} table={table} foreignKeys={foreignKeys} /></td>)}</tr>)}</tbody>
      </table>
      {!loading && rows.length === 0 && <div className="schema-blank compact">This table is empty.</div>}
    </div>
    <footer className="schema-row-footer"><span>{focus ? "Focused row" : `${rows.length} row${rows.length === 1 ? "" : "s"} loaded`}</span>{focus && <button className="ghost" onClick={() => navigate(schemaPath(table.name, "rows"))}>Show all rows</button>}{page?.next_cursor && !focus && <button className="secondary" disabled={loading} onClick={() => void load(page.next_cursor!, true)}>{loading ? "Loading…" : "Load next 50"}</button>}</footer>
  </div>;
}

function CellValue({ value, column, table, foreignKeys }: { value: string | null; column: string; table: SchemaTable; foreignKeys: SchemaForeignKey[] }) {
  if (value === null) return <span className="schema-null">NULL</span>;
  const fk = foreignKeys.find((edge) => edge.source_table === table.name && edge.source_columns.length === 1 && edge.source_columns[0] === column);
  const objectReference = (table.name === "objects" && column === "id") || (fk?.target_table === "objects" && fk.target_columns[0] === "id") || (table.classification === "subtype" && column === "object_id");
  const target = objectReference ? objectPath(value) : fk ? schemaRowPath(fk.target_table, fk.target_columns[0], value) : null;
  const long = value.length > 80 || value.includes("\n");
  return <div className="schema-cell">
    {long ? <details><summary>{value.slice(0, 80)}…</summary><pre>{value}</pre></details> : target ? <button className="schema-cell-link" onClick={() => navigate(target)} title={value}>{value}</button> : <span title={value}>{value}</span>}
    <button className="schema-copy" onClick={() => void navigator.clipboard?.writeText(value)} aria-label={`Copy ${column}`} title="Copy value">⧉</button>
  </div>;
}

function rowKey(row: Record<string, string | null>, table: SchemaTable, index: number) {
  const primary = table.constraints.find((constraint) => constraint.kind === "primary_key")?.columns ?? [];
  return primary.map((column) => row[column]).join("|") || String(index);
}

function tableLabel(name: string) {
  return name.split("_").map((part) => part.charAt(0).toUpperCase() + part.slice(1)).join(" ");
}

function formatCount(value: number) {
  return value < 10_000 ? String(value) : countFormatter.format(value);
}
