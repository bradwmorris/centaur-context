import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "./api";
import { objectPath, navigate, schemaPath, schemaRowPath, schemaView } from "./routing";
import type { SchemaColumn, SchemaForeignKey, SchemaRowPage, SchemaSnapshot, SchemaTable, SchemaViewMode } from "./types";

interface Props {
  selectedTable: string | null;
}

const countFormatter = new Intl.NumberFormat(undefined, { notation: "compact", maximumFractionDigits: 1 });

export function SchemaWorkspace({ selectedTable }: Props) {
  const [snapshot, setSnapshot] = useState<SchemaSnapshot | null>(null);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mode = schemaView(window.location.pathname);

  const refresh = useCallback(async () => {
    try {
      const next = await api.schema();
      setSnapshot(next);
      setError(null);
      if (selectedTable && !next.tables.some((table) => table.name === selectedTable)) {
        navigate(schemaPath());
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "The schema could not be loaded.");
    } finally {
      setLoading(false);
    }
  }, [selectedTable]);

  useEffect(() => {
    void refresh();
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    const interval = window.setInterval(() => void refresh(), 60_000);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.clearInterval(interval);
    };
  }, [refresh]);

  const table = snapshot?.tables.find((item) => item.name === selectedTable) ?? null;
  const visibleTables = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return snapshot?.tables.filter((item) => !needle || item.name.toLowerCase().includes(needle)) ?? [];
  }, [query, snapshot]);

  return <section className="schema-workspace" aria-label="Database schema">
    <aside className="schema-catalog">
      <div className="schema-catalog-head">
        <div><strong>Schema</strong><span>{snapshot?.tables.length ?? 0} tables</span></div>
        <button className="schema-refresh" onClick={() => void refresh()} aria-label="Refresh schema" title="Refresh schema">↻</button>
      </div>
      <label className="schema-search"><span aria-hidden="true">⌕</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Find a table" aria-label="Find a table" /></label>
      <select className="schema-mobile-table-select" aria-label="Choose a schema table" value={selectedTable ?? ""} onChange={(event) => navigate(event.target.value ? schemaPath(event.target.value) : schemaPath())}>
        <option value="">Schema map</option>
        {snapshot?.tables.map((item) => <option key={item.name} value={item.name}>{tableLabel(item.name)}</option>)}
      </select>
      <nav aria-label="Schema tables" className="schema-table-list">
        {(["canonical", "subtype", "supporting"] as const).map((classification) => {
          const tables = visibleTables.filter((item) => item.classification === classification);
          if (tables.length === 0) return null;
          return <section key={classification} className="schema-table-group">
            <h2>{classification === "canonical" ? "Canonical" : classification === "subtype" ? "Object subtypes" : "Supporting"}</h2>
            {tables.map((item) => <button key={item.name} className={item.name === selectedTable ? "active" : ""} aria-current={item.name === selectedTable ? "page" : undefined} onClick={() => navigate(schemaPath(item.name))}>
              <span>{tableLabel(item.name)}</span><small>{formatCount(item.estimated_row_count)}</small>
            </button>)}
          </section>;
        })}
      </nav>
      {snapshot && <small className="schema-fingerprint" title={snapshot.fingerprint}>Live · {snapshot.fingerprint.slice(0, 7)}</small>}
    </aside>

    <div className="schema-main">
      <header className="schema-toolbar">
        <div><h1>{table ? tableLabel(table.name) : "Schema map"}</h1><p>{table ? `${table.classification} table · ${table.columns.length} columns` : "Live structure of the Centaur Context database"}</p></div>
        <div className="schema-view-switch" aria-label="Schema view">
          <ViewButton mode="map" current={mode} table={selectedTable}>Map</ViewButton>
          <ViewButton mode="structure" current={mode} table={selectedTable} disabled={!table}>Structure</ViewButton>
          <ViewButton mode="rows" current={mode} table={selectedTable} disabled={!table}>Rows</ViewButton>
        </div>
      </header>
      {error && <div className="schema-message error">{error}<button onClick={() => setError(null)} aria-label="Dismiss error">×</button></div>}
      {loading && !snapshot ? <div className="schema-blank">Reading the live schema…</div> : !snapshot ? <div className="schema-blank">Schema unavailable.</div> : mode === "map" ? <SchemaMap snapshot={snapshot} /> : !table ? <div className="schema-blank">Choose a table to inspect.</div> : mode === "structure" ? <TableStructure table={table} foreignKeys={snapshot.foreign_keys} /> : <TableRows table={table} fingerprint={snapshot.fingerprint} foreignKeys={snapshot.foreign_keys} />}
    </div>
  </section>;
}

function ViewButton({ mode, current, table, disabled, children }: { mode: SchemaViewMode; current: SchemaViewMode; table: string | null; disabled?: boolean; children: string }) {
  return <button className={mode === current ? "active" : ""} aria-pressed={mode === current} disabled={disabled} onClick={() => navigate(schemaPath(table ?? undefined, mode))}>{children}</button>;
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
    <RelationshipList foreignKeys={snapshot.foreign_keys} />
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

function TableStructure({ table, foreignKeys }: { table: SchemaTable; foreignKeys: SchemaForeignKey[] }) {
  const relationships = foreignKeys.filter((edge) => edge.source_table === table.name || edge.target_table === table.name);
  return <div className="schema-structure">
    <section className="schema-summary">
      <div><span>Classification</span><strong>{table.classification}</strong></div>
      <div><span>Columns</span><strong>{table.columns.length}</strong></div>
      <div><span>Rows</span><strong>≈ {formatCount(table.estimated_row_count)}</strong></div>
      <div><span>Relationships</span><strong>{relationships.length}</strong></div>
    </section>
    <section className="schema-section">
      <h2>Columns</h2>
      <div className="schema-columns">
        {table.columns.map((column) => <ColumnRow key={column.name} column={column} table={table} foreignKeys={foreignKeys} />)}
      </div>
    </section>
    <section className="schema-section">
      <h2>Relationships</h2>
      {relationships.length ? <RelationshipList foreignKeys={relationships} selected={table.name} /> : <p className="schema-muted">No foreign-key relationships.</p>}
    </section>
    <section className="schema-section schema-constraints">
      <h2>Constraints</h2>
      {table.constraints.map((constraint) => <details key={constraint.name}><summary><span>{constraint.name}</span><small>{constraint.kind.replaceAll("_", " ")}</small></summary><code>{constraint.definition}</code></details>)}
    </section>
  </div>;
}

function ColumnRow({ column, table, foreignKeys }: { column: SchemaColumn; table: SchemaTable; foreignKeys: SchemaForeignKey[] }) {
  const constraints = table.constraints.filter((constraint) => constraint.columns.includes(column.name));
  const isPk = constraints.some((constraint) => constraint.kind === "primary_key");
  const isUnique = constraints.some((constraint) => constraint.kind === "unique");
  const fk = foreignKeys.find((edge) => edge.source_table === table.name && edge.source_columns.includes(column.name));
  return <div className="schema-column">
    <div><strong>{column.name}</strong><code>{column.data_type}</code></div>
    <div className="schema-badges">{isPk && <span>PK</span>}{fk && <button onClick={() => navigate(schemaPath(fk.target_table))}>FK → {fk.target_table}</button>}{isUnique && <span>Unique</span>}{column.nullable && <span>Nullable</span>}{column.identity && <span>Identity</span>}{column.generated && <span>Generated</span>}{column.default && <span title={column.default}>Default</span>}</div>
  </div>;
}

function RelationshipList({ foreignKeys, selected }: { foreignKeys: SchemaForeignKey[]; selected?: string }) {
  return <ul className="schema-relationships" aria-label="Schema relationships">{foreignKeys.map((edge) => {
    const outward = !selected || edge.source_table === selected;
    const target = outward ? edge.target_table : edge.source_table;
    const sourceColumns = outward ? edge.source_columns : edge.target_columns;
    const targetColumns = outward ? edge.target_columns : edge.source_columns;
    return <li key={`${edge.source_table}:${edge.name}`}><button onClick={() => navigate(schemaPath(target))}>
      <span><strong>{outward ? edge.source_table : edge.target_table}</strong><code>{sourceColumns.join(", ")}</code></span>
      <b aria-label={edge.one_to_one_subtype ? "one-to-one subtype relationship" : edge.nullable ? "optional foreign key relationship" : "required foreign key relationship"}>{edge.one_to_one_subtype ? "1:1" : edge.nullable ? "0..1 →" : "→"}</b>
      <span><strong>{target}</strong><code>{targetColumns.join(", ")}</code></span>
    </button></li>;
  })}</ul>;
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
