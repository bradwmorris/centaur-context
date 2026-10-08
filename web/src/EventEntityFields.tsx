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
export function EventEntityDetail({ id, kind, revision, onChanged }: { id: string; kind: string; revision: number; onChanged: () => Promise<void> }) {
  const [fields, setFields] = useState<SubtypeFields | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => { let active = true; api.readSubtype(id).then(value => { if (active) setFields(value); }).catch(e => { if (active) setError(String(e)); }); return () => { active = false; }; }, [id, revision]);
  return <section><h2>{kind === "event" ? "Occurrence" : "Classifications"}</h2>{error && <p role="alert">{error}</p>}{fields && <form key={`${id}:${revision}`} onSubmit={async e => {
    e.preventDefault(); const changes = fieldsFromForm(kind, new FormData(e.currentTarget)); setBusy(true); setError("");
    try { await api.applyCorrections([{ operation: "update_object", object_id: id, expected_revision: revision, changes }]); await onChanged(); } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }}>{kind === "event" ? <EventFields initial={fields} /> : <EntityFields initial={fields} />}<button disabled={busy}>{busy ? "Saving…" : "Save properties"}</button></form>}</section>;
}
export function CategoryManager() {
  const [categories, setCategories] = useState<EntityCategory[]>([]);
  const [editing, setEditing] = useState<EntityCategory | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const reload = async () => setCategories((await api.entityCategories()).categories);
  useEffect(() => { void reload().catch(e => setError(String(e))); }, []);
  const mutate = async (operation: Record<string, unknown>) => { setBusy(true); setError(""); try { await api.applyCorrections([operation]); await reload(); window.dispatchEvent(new Event("entity-categories-changed")); setEditing(null); } catch (e) { setError(String(e)); } finally { setBusy(false); } };
  return <details className="category-manager"><summary>Manage category definitions</summary><p>Reuse defined categories. Category names classify Entities; they are not research Themes.</p>{error && <p role="alert">{error}</p>}
    <ul>{categories.map(c => <li key={c.id}><strong>{c.label}</strong> — {c.definition} {c.aliases.length > 0 && <small>Also: {c.aliases.join(", ")}</small>}<button onClick={() => setEditing(c)}>Edit</button><button disabled={busy} onClick={() => void mutate({ operation: c.archived_at ? "restore_entity_category" : "archive_entity_category", category_id: c.id, expected_revision: c.revision })}>{c.archived_at ? "Restore" : "Archive"}</button></li>)}</ul>
    <form key={editing?.id ?? "new"} onSubmit={e => { e.preventDefault(); const d = new FormData(e.currentTarget); const changes = { label: d.get("label"), definition: d.get("definition"), aliases: String(d.get("aliases") ?? "").split(",").map(v => v.trim()).filter(Boolean), legacy_kind: d.get("legacy_kind") }; void mutate(editing ? { operation: "update_entity_category", category_id: editing.id, expected_revision: editing.revision, changes } : { operation: "create_entity_category", slug: d.get("slug"), ...changes }); }}>
      <h3>{editing ? `Edit ${editing.label}` : "Add a category"}</h3>
      {!editing && <label>Stable slug<input name="slug" required pattern="[a-z][a-z0-9]*(-[a-z0-9]+)*" maxLength={100} /></label>}
      <label>Label<input name="label" defaultValue={editing?.label} required maxLength={100} /></label><label>Definition<textarea name="definition" defaultValue={editing?.definition} required maxLength={1000} /></label>
      <label>Alternative labels (comma separated)<input name="aliases" defaultValue={editing?.aliases.join(", ")} /></label><label>Legacy classification<select name="legacy_kind" defaultValue={editing?.legacy_kind ?? "other"}>{["person","organization","product","project","publication","place","concept","other"].map(k => <option key={k}>{k}</option>)}</select></label>
      <button disabled={busy}>{editing ? "Save definition" : "Create category"}</button>{editing && <button type="button" onClick={() => setEditing(null)}>Cancel edit</button>}
    </form>
  </details>;
}
