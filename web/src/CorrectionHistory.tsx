import { useState } from "react";
import { api } from "./api";

type Assertion = { id: string; target_type: string; target_id: string; reason: string; actor_id: string; representation: Record<string, unknown>; supersedes_correction_id: string | null };
type RecordRef = { id: string; title?: string; content?: string; action?: string; run_id?: string };
type Snapshot = { object: { id: string; kind: string; revision: number; lifecycle: string }; corrections: Assertion[]; artifacts?: RecordRef[]; events?: RecordRef[]; messages?: RecordRef[] };

export function CorrectionHistory({ id, kind, onChanged }: { id: string; kind: string; onChanged: () => Promise<void> }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [open, setOpen] = useState(false);
  const load = async () => {
    const data = await api.readCorrections(id, kind === "chat") as { objects: Snapshot[] };
    setSnapshot(data.objects[0]);
  };
  const reveal = async () => {
    setOpen(true); setBusy(true); setError("");
    try { await load(); } catch (cause) { setError(String(cause)); } finally { setBusy(false); }
  };
  const save = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!snapshot) return;
    const form = event.currentTarget;
    const data = new FormData(form);
    const [target_type, target_id] = String(data.get("target")).split(":");
    const text = String(data.get("content") ?? "").trim();
    setBusy(true); setError("");
    try {
      await api.applyCorrections([{ operation: "correct_evidence", object_id: id, expected_revision: snapshot.object.revision, target_type, target_id, reason: String(data.get("reason")), representation: text ? { content: text } : {} }]);
      form.reset(); await load(); await onChanged();
    } catch (cause) { setError(String(cause)); } finally { setBusy(false); }
  };
  const restore = async () => {
    if (!snapshot) return;
    setBusy(true); setError("");
    try { await api.applyCorrections([{ operation: "restore_object", object_id: id, expected_revision: snapshot.object.revision }]); await load(); await onChanged(); }
    catch (cause) { setError(String(cause)); } finally { setBusy(false); }
  };
  const superseded = new Set(snapshot?.corrections.flatMap((item) => item.supersedes_correction_id ? [item.supersedes_correction_id] : []) ?? []);
  return <section aria-label="Evidence corrections">
    <button type="button" className="secondary" disabled={busy} onClick={() => void reveal()}>Review or correct evidence</button>
    {open && <>
      <p>Corrections retain the original evidence and record who made the change. Leave corrected text empty to withdraw an earlier correction.</p>
      {error && <p role="alert">{error} Reload before retrying a revision conflict.</p>}
      {snapshot && <>
        {snapshot.object.lifecycle === "archived" && <button type="button" disabled={busy} onClick={() => void restore()}>Restore record</button>}
        <form onSubmit={(event) => void save(event)}>
          <label>Evidence<select name="target" required defaultValue={`object:${id}`}>
            <option value={`object:${id}`}>Record identity or historical metadata</option>
            {snapshot.artifacts?.map((item) => <option key={item.id} value={`artifact:${item.id}`}>Artifact: {item.title ?? item.id}</option>)}
            {snapshot.messages?.map((item) => <option key={item.id} value={`message:${item.id}`}>Message: {item.content?.slice(0, 80) ?? item.id}</option>)}
            {snapshot.events?.map((item) => <option key={item.id} value={`event:${item.id}`}>Event: {item.action} {item.id.slice(0, 8)}</option>)}
          </select></label>
          <label>Corrected representation<textarea name="content" maxLength={20000} /></label>
          <label>Reason<textarea name="reason" required maxLength={2000} /></label>
          <button type="submit" disabled={busy}>Save correction</button>
        </form>
        {snapshot.corrections.map((item) => <article key={item.id}><strong>{superseded.has(item.id) ? "Historical" : "Current"} {item.target_type} correction by {item.actor_id}</strong><p>{item.reason}</p><pre>{JSON.stringify(item.representation, null, 2)}</pre></article>)}
      </>}
    </>}
  </section>;
}
