import "./eventEntity.css";
import { useEffect, useState } from "react";
import { api } from "./api";

export interface EntityCategory { id: string; slug: string; label: string; definition: string; aliases: string[]; legacy_kind: string; revision: number; archived_at: string | null }
export type SubtypeFields = Record<string, unknown>;
export function fieldsFromForm(kind: string, data: FormData): SubtypeFields {
  if (kind === "entity") return { category_ids: data.getAll("category_ids"), primary_category_id: data.get("primary_category_id") };
  const fields: SubtypeFields = { timezone: data.get("timezone") || null };
  for (const endpoint of ["starts_at", "ends_at"]) {
    fields[endpoint] = data.get(endpoint) || null;
    fields[`${endpoint}_precision`] = data.get(endpoint) ? data.get(`${endpoint}_precision`) : null;
  }
  return fields;
}
export function EventFields({ initial = {} }: { initial?: SubtypeFields }) {
  return <fieldset className="typed-properties"><legend>Event timing</legend><p>Use only supported timing. Leave unknown endpoints blank. Calendar range ends are inclusive.</p>
    {["starts_at", "ends_at"].map((endpoint) => <label key={endpoint}>{endpoint === "starts_at" ? "Start" : "End"}
      <select name={`${endpoint}_precision`} aria-label={`${endpoint} precision`} defaultValue={String(initial[`${endpoint}_precision`] ?? "day")}>
        <option value="year">Year (YYYY)</option><option value="month">Month (YYYY-MM)</option><option value="day">Day (YYYY-MM-DD)</option><option value="instant">Instant (RFC3339 with offset)</option>
      </select><input name={endpoint} aria-label={endpoint === "starts_at" ? "Event start" : "Event end"} defaultValue={String(initial[endpoint] ?? "")} placeholder="Unknown" />
    </label>)}
    <label>Timezone (optional IANA name)<input name="timezone" defaultValue={String(initial.timezone ?? "")} placeholder="Australia/Sydney" /></label>
  </fieldset>;
}
export function EntityFields({ initial = {} }: { initial?: SubtypeFields }) {
  const [categories, setCategories] = useState<EntityCategory[]>([]);
  const [selected, setSelected] = useState<string[]>((initial.category_ids as string[]) ?? []);
  const [primary, setPrimary] = useState(String(initial.primary_category_id ?? ""));
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    const reload = () => { void api.entityCategories().then(c => { if (active) setCategories(c.categories); }).catch(e => { if (active) setError(String(e)); }); };
    reload(); window.addEventListener("entity-categories-changed", reload);
    return () => { active = false; window.removeEventListener("entity-categories-changed", reload); };
  }, []);
  return <fieldset className="typed-properties"><legend>Entity classifications</legend>{error && <p role="alert">{error}</p>}
    {categories.filter(c => !c.archived_at || selected.includes(c.id)).map(c => <label className="category-choice" key={c.id} title={c.definition}>
      <input type="checkbox" name="category_ids" value={c.id} checked={selected.includes(c.id)} onChange={e => {
        const next = e.target.checked ? [...selected, c.id] : selected.filter(id => id !== c.id); setSelected(next);
        if (!next.includes(primary)) setPrimary(next[0] ?? "");
      }} /><span>{c.label}{c.archived_at ? " (archived)" : ""}</span><small>{c.definition}</small>
    </label>)}
    <label>Primary classification<select name="primary_category_id" required value={primary} onChange={e => setPrimary(e.target.value)}><option value="">Select a classification</option>{categories.filter(c => selected.includes(c.id)).map(c => <option key={c.id} value={c.id}>{c.label}</option>)}</select></label>
  </fieldset>;
}
function EventDetail({ id }: { id: string }) {
  const [fields, setFields] = useState<SubtypeFields | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    void api.readSubtype(id).then(value => { if (active) setFields(value); }).catch(cause => { if (active) setError(String(cause)); });
    return () => { active = false; };
  }, [id]);
  const timing = [
    ["Start", fields?.starts_at],
    ["End", fields?.ends_at],
    ["Timezone", fields?.timezone],
  ].filter((entry): entry is [string, string] => typeof entry[1] === "string" && entry[1].length > 0);
  return <section className="properties-block event-occurrence" aria-label="Occurrence">
    <h2>Occurrence</h2>
    {error ? <p role="alert">Could not load occurrence: {error}</p> : fields === null ? <p className="muted" role="status">Loading occurrence…</p> : timing.length === 0 ? <p className="muted">Timing not recorded.</p> :
      <dl className="event-occurrence-values">{timing.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{label === "Timezone" ? value : <time dateTime={value}>{value}</time>}</dd></div>)}</dl>}
  </section>;
}

export function EventEntityDetail(props: { id: string; kind: string; revision: number }) {
  return props.kind === "entity"
    ? <EntityKind key={`${props.id}:${props.revision}`} id={props.id} />
    : <EventDetail key={`${props.id}:${props.revision}`} id={props.id} />;
}

function EntityKind({ id }: { id: string }) {
  const [kind, setKind] = useState<string | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    void Promise.all([api.readSubtype(id), api.entityCategories()]).then(([fields, catalogue]) => {
      if (!active) return;
      const ids = (fields.category_ids as string[] | undefined) ?? [];
      const primary = typeof fields.primary_category_id === "string" && ids.includes(fields.primary_category_id)
        ? fields.primary_category_id : ids[0];
      const label = catalogue.categories.find(category => category.id === primary)?.label;
      const legacyKind = typeof fields.entity_kind === "string" ? fields.entity_kind.trim() : "";
      setKind(label || legacyKind || (primary ? "Kind unavailable" : "Kind not recorded"));
    }).catch(cause => { if (active) setError(String(cause)); });
    return () => { active = false; };
  }, [id]);
  return <div className="entity-kind" role="group" aria-label="Entity kind">
    {error ? <p role="alert">Could not load kind: {error}</p> : kind === null ? <p className="muted" role="status">Loading kind…</p> : <span className="visual-badge">{kind}</span>}
  </div>;
}
