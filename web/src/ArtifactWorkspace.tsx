import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import { ApiError, api } from "./api";
import { artifactDocuments, artifactLabel, documentKey, editableText, mediaType, readableText, safeArtifactUri, verifyBody } from "./artifactDocuments";
import { artifactPath, detailPath, interceptNavigation, navigate, workingDocumentPath } from "./routing";
import type { Artifact, SharedObject } from "./types";
import "./artifactWorkspace.css";

type ParentSection = "objects" | "sources" | "notes";
type Target = { kind: "latest" | "exact"; id: string };
const WINDOW_SIZE = 20_000;

function errorText(error: unknown): string {
  if (error instanceof ApiError && (error.status === 403 || error.status === 401)) return `Access denied: ${error.message}`;
  if (error instanceof ApiError && error.status === 409) return "This Object changed while you were editing. Your draft is kept; reload the document before saving again.";
  return error instanceof Error ? error.message : String(error);
}

async function fullBody(artifact: Artifact): Promise<string> {
  let offset = 0;
  let text = "";
  for (;;) {
    const window = await api.artifactContent(artifact.id, offset, WINDOW_SIZE);
    if (window.id !== artifact.id || window.offset !== offset) throw new Error("Artifact content changed during loading.");
    text += window.text;
    if (window.next_offset === null) break;
    if (window.next_offset <= offset || window.next_offset !== offset + Array.from(window.text).length) throw new Error("Artifact content window is incomplete.");
    offset = window.next_offset;
  }
  await verifyBody(artifact, text);
  return text;
}

async function consistentSnapshot(objectId: string): Promise<{ object: SharedObject; artifacts: Artifact[] }> {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const before = await api.object(objectId);
    const artifacts = await api.artifacts(objectId);
    const after = await api.object(objectId);
    if (before.revision === after.revision) return { object: after, artifacts };
  }
  throw new Error("The Object changed during loading. Retry to get a consistent document version.");
}

function displayType(artifact: Artifact): string {
  return artifact.kind === "research_notes" ? "Research documents" : artifact.kind.replaceAll("_", " ");
}

function artifactHref(section: ParentSection, objectId: string, artifact: Artifact, key?: string | null): string {
  return key === null || key === undefined ? artifactPath(section, objectId, artifact.id) : workingDocumentPath(section, objectId, key);
}

function ArtifactLink({ href, children, className }: { href: string; children: React.ReactNode; className?: string }) {
  return <a className={className} href={href} onClick={(event) => interceptNavigation(event, href)}>{children}</a>;
}

export function Artifacts({ section, objectId, artifacts, onCreated }: { section: ParentSection; objectId: string; artifacts: Artifact[]; onCreated: () => Promise<void> }) {
  const [form, setForm] = useState<"notes" | "generic" | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pendingCreate = useRef<{ signature: string; idempotencyKey: string; documentKey: string | null; expectedRevision: number | null } | null>(null);
  const documents = useMemo(() => artifactDocuments(artifacts), [artifacts]);
  const openForm = (next: "notes" | "generic") => { pendingCreate.current = null; setError(null); setForm(next); };
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setBusy(true); setError(null);
    const data = new FormData(event.currentTarget);
    const isNotes = form === "notes";
    const signature = JSON.stringify({ form, values: [...data.entries()] });
    const previous = pendingCreate.current;
    const attempt = previous?.signature === signature ? previous : {
      signature, idempotencyKey: crypto.randomUUID(), documentKey: isNotes ? crypto.randomUUID() : null,
      expectedRevision: null,
    };
    pendingCreate.current = attempt;
    const key = attempt.documentKey;
    try {
      const { object, artifacts: known } = await consistentSnapshot(objectId);
      const existing = known.find((item) => isNotes
        ? documentKey(item) === key && item.kind === "research_notes"
        : item.metadata.creation_key === attempt.idempotencyKey);
      if (existing) {
        if (await fullBody(existing) !== String(data.get("body"))) throw new Error("An earlier submission returned different content. Check the artifact history.");
        await onCreated(); setForm(null); pendingCreate.current = null;
        navigate(artifactHref(section, objectId, existing, key));
        return;
      }
      attempt.expectedRevision ??= object.revision;
      const kind = isNotes ? "research_notes" : String(data.get("kind")).trim();
      const body = String(data.get("body"));
      const genericDocumentKey = String(data.get("document_key") ?? "").trim() || "default";
      const selectedPredecessor = String(data.get("predecessor") ?? "").trim();
      const predecessor = isNotes ? null : selectedPredecessor || (kind === "research_notes" ? artifactDocuments(known).find((item) => item.key === genericDocumentKey)?.latest.id ?? null : null);
      const captureOutcome = isNotes ? "complete" : String(data.get("capture_outcome"));
      const capturedAt = String(data.get("captured_at") ?? "");
      const artifact = await api.createArtifact(objectId, {
        expected_revision: attempt.expectedRevision,
        kind,
        title: String(data.get("title")).trim() || null,
        content: body,
        media_type: isNotes ? "text/markdown" : "text/plain",
        language: isNotes ? null : String(data.get("language") ?? "").trim() || null,
        captured_at: capturedAt ? new Date(capturedAt).toISOString() : null,
        capture_outcome: captureOutcome,
        capture_reason: captureOutcome === "complete" ? null : String(data.get("capture_reason") ?? "").trim() || null,
        expected_size_bytes: null,
        metadata: isNotes ? { document_key: key, source_type: "human_paste" } : { source_type: "human_paste", creation_key: attempt.idempotencyKey, ...(kind === "research_notes" ? { document_key: genericDocumentKey, predecessor_artifact_id: predecessor } : {}) },
        supersedes_artifact_id: predecessor,
      }, attempt.idempotencyKey);
      if (artifact.kind !== kind || artifact.object_id !== objectId || (isNotes ? documentKey(artifact) !== key : artifact.metadata.creation_key !== attempt.idempotencyKey) || await fullBody(artifact) !== body) {
        throw new Error("The server returned a different artifact. Check the history before retrying.");
      }
      await onCreated();
      setForm(null);
      pendingCreate.current = null;
      navigate(artifactHref(section, objectId, artifact, key));
    } catch (cause) { setError(errorText(cause)); }
    finally { setBusy(false); }
  };
  return <section className="artifact-section" aria-label="Artifacts">
    <div className="artifact-section-head"><h2>Artifacts</h2><div className="artifact-actions"><button className="secondary" type="button" onClick={() => openForm("notes")}>+ Add research document</button><button className="ghost" type="button" onClick={() => openForm("generic")}>Add artifact</button></div></div>
    {form && <form className="artifact-create-form" onSubmit={(event) => void submit(event)}>
      <h3>{form === "notes" ? "New research document" : "New artifact"}</h3>
      {form === "generic" && <div className="artifact-create-fields"><label>Kind<input name="kind" required maxLength={100} defaultValue="transcript" /></label><label>Document key for research documents<input name="document_key" maxLength={100} placeholder="default" /></label><label>Captured<input name="captured_at" type="datetime-local" /></label><label>Language<input name="language" maxLength={35} placeholder="en" /></label><label>Capture outcome<select name="capture_outcome" defaultValue="complete"><option value="complete">Complete</option><option value="incomplete">Incomplete</option><option value="unavailable">Unavailable</option><option value="paywalled">Paywalled</option><option value="disallowed">Disallowed</option><option value="too_large">Too large</option><option value="unsupported">Unsupported</option></select></label><label>Reason when not complete<input name="capture_reason" maxLength={1000} /></label><label>Supersedes artifact<select name="predecessor" defaultValue=""><option value="">None / latest research document with this key</option>{artifacts.map((item) => <option key={item.id} value={item.id}>{artifactLabel(item)} · {item.id.slice(0, 8)}</option>)}</select></label></div>}
      <label>Title<input name="title" maxLength={300} placeholder={form === "notes" ? "Research documents" : "Artifact title"} /></label>
      <label>Body<textarea name="body" required rows={8} aria-label="Artifact body" /></label>
      <div className="artifact-actions"><button className="ghost" type="button" onClick={() => { setForm(null); pendingCreate.current = null; }}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Saving…" : "Save"}</button></div>
    </form>}
    {documents.length ? <div className="artifact-list">{documents.map(({ key, latest, history }) => <ArtifactLink key={key === null ? latest.id : `working:${key}`} className="artifact-row" href={artifactHref(section, objectId, latest, key)}><span className="artifact-row-title">{artifactLabel(latest)}</span><span className="artifact-row-meta">{displayType(latest)}{history.length > 1 ? ` · ${history.length} versions` : ""}{latest.capture_outcome !== "complete" ? ` · ${latest.capture_outcome}` : ""}</span><span aria-hidden="true">›</span></ArtifactLink>)}</div> : <p className="muted">No artifacts yet.</p>}
    {error && <p className="form-error" role="alert">{error}</p>}
  </section>;
}

function Markdown({ text }: { text: string }) {
  return <div className="artifact-markdown"><ReactMarkdown skipHtml components={{ img: ({ alt }) => <span>[Image: {alt ?? "untitled"}]</span> }}>{text}</ReactMarkdown></div>;
}

export function ArtifactReader({ section, objectId, target, refreshKey }: { section: ParentSection; objectId: string; target: Target; refreshKey: number }) {
  const [snapshot, setSnapshot] = useState<{ object: SharedObject; artifacts: Artifact[] } | null>(null);
  const [loadingError, setLoadingError] = useState<string | null>(null);
  const [body, setBody] = useState<string | null>(null);
  const [bodyError, setBodyError] = useState<string | null>(null);
  const [bodyUnavailable, setBodyUnavailable] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [panel, setPanel] = useState<"history" | "details" | null>(null);
  const [editBaseRevision, setEditBaseRevision] = useState<number | null>(null);
  const pending = useRef<{ baseId: string; text: string; key: string } | null>(null);
  const path = target.kind === "latest" ? workingDocumentPath(section, objectId, target.id) : artifactPath(section, objectId, target.id);
  const parentPath = detailPath(section, objectId);
  const dirty = editing && body !== null && draft !== body;

  const load = useCallback(async () => {
    setLoadingError(null);
    try {
      setSnapshot(await consistentSnapshot(objectId));
    } catch (cause) { setLoadingError(errorText(cause)); }
  }, [objectId]);
  useEffect(() => { if (!dirty) void load(); }, [load, refreshKey]);

  const document = useMemo(() => snapshot?.artifacts ? artifactDocuments(snapshot.artifacts).find((item) => target.kind === "latest" ? item.key === target.id : item.history.some((artifact) => artifact.id === target.id)) : null, [snapshot, target.kind, target.id]);
  const artifact = section === "objects" && snapshot && snapshot.object.kind !== "source" && snapshot.object.kind !== "note"
    ? undefined : target.kind === "exact" ? document?.history.find((item) => item.id === target.id) : document?.latest;
  const latest = Boolean(artifact && document && artifact.id === document.latest.id && target.kind === "latest");

  useEffect(() => {
    let cancelled = false;
    setBody(null); setBodyError(null); setBodyUnavailable(false); setEditing(false); setSaveError(null); setPanel(null);
    if (!artifact) return;
    if (!readableText(artifact)) { setBodyUnavailable(true); return; }
    void fullBody(artifact).then((text) => { if (!cancelled) { setBody(text); setDraft(text); } }).catch((cause) => {
      if (cancelled) return;
      if (cause instanceof ApiError && cause.status === 404 && artifact.uri) setBodyUnavailable(true);
      else setBodyError(errorText(cause));
    });
    return () => { cancelled = true; };
  }, [artifact?.id]);

  useEffect(() => {
    if (!dirty) return;
    let approvedPath: string | null = null;
    const beforeUnload = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ""; };
    const beforeNavigate = (event: Event) => {
      if (!window.confirm("Discard unsaved working-notes changes?")) event.preventDefault();
      else approvedPath = (event as CustomEvent<string>).detail;
    };
    const onPop = () => {
      if (`${window.location.pathname}` === path) return;
      if (`${window.location.pathname}${window.location.search}` === approvedPath) return;
      if (!window.confirm("Discard unsaved working-notes changes?")) {
        window.history.replaceState({}, "", path);
        window.dispatchEvent(new PopStateEvent("popstate"));
      }
    };
    window.addEventListener("beforeunload", beforeUnload);
    window.addEventListener("beforeappnavigate", beforeNavigate);
    window.addEventListener("popstate", onPop);
    return () => { window.removeEventListener("beforeunload", beforeUnload); window.removeEventListener("beforeappnavigate", beforeNavigate); window.removeEventListener("popstate", onPop); };
  }, [dirty, path]);

  const save = async () => {
    if (!artifact || !snapshot || editBaseRevision === null || body === null || !document || draft === body || !latest) { setEditing(false); return; }
    setBusy(true); setSaveError(null);
    const key = document.key;
    if (key === null) { setBusy(false); return; }
    try {
      const { artifacts: freshArtifacts, object: freshObject } = await consistentSnapshot(objectId);
      const freshDocument = artifactDocuments(freshArtifacts).find((item) => item.key === key);
      const retry = pending.current;
      const idempotencyKey = retry?.baseId === artifact.id && retry.text === draft ? retry.key : crypto.randomUUID();
      pending.current = { baseId: artifact.id, text: draft, key: idempotencyKey };
      const expected = {
        expected_revision: editBaseRevision,
        kind: "research_notes", title: artifact.title, content: draft,
        media_type: artifact.media_type, language: artifact.language,
        capture_outcome: "complete", capture_reason: null, expected_size_bytes: null,
        metadata: { ...artifact.metadata, document_key: key, predecessor_artifact_id: artifact.id },
        supersedes_artifact_id: artifact.id,
      };
      // Reconcile an uncertain earlier submission before rejecting its now-stale
      // Object revision. A retry uses the same key and cannot append twice.
      let created: Artifact | undefined = retry?.baseId === artifact.id && retry.text === draft
        ? freshArtifacts.find((item) => item.kind === "research_notes" && documentKey(item) === key && item.supersedes_artifact_id === artifact.id)
        : undefined;
      if (!created) {
        if (freshObject.revision !== editBaseRevision || freshDocument?.latest.id !== artifact.id) {
          throw new Error("This Object or working document changed while you were editing. Your draft is kept; reload before saving.");
        }
        try { created = await api.createArtifact(objectId, expected, idempotencyKey); }
        catch (cause) {
          // A response may be lost after commit. Read back before offering a retry.
          const after = await api.artifacts(objectId);
          const successor = after.find((item) => item.kind === "research_notes" && documentKey(item) === key && item.supersedes_artifact_id === artifact.id);
          if (!successor) throw cause;
          created = successor;
        }
      }
      if (created.id === artifact.id || created.object_id !== objectId || created.kind !== "research_notes" || documentKey(created) !== key || created.supersedes_artifact_id !== artifact.id || created.media_type !== artifact.media_type) {
        throw new Error("The server did not return the expected successor. Your draft is kept.");
      }
      const savedBody = await fullBody(created);
      if (savedBody !== draft) throw new Error("The saved content differs from your draft. Your draft is kept.");
      const { artifacts: refreshed, object } = await consistentSnapshot(objectId);
      if (artifactDocuments(refreshed).find((item) => item.key === key)?.latest.id !== created.id) throw new Error("A newer revision is present. Your draft is kept.");
      setSnapshot({ object, artifacts: refreshed }); setBody(draft); setEditing(false); setEditBaseRevision(null); pending.current = null;
    } catch (cause) { setSaveError(errorText(cause)); }
    finally { setBusy(false); }
  };

  return <article className="artifact-reader">
    <ArtifactLink className="artifact-back" href={parentPath}>← Back to Object</ArtifactLink>
    {loadingError && <p className="form-error" role="alert">{loadingError} <button type="button" onClick={() => void load()}>Retry</button></p>}
    {!snapshot && !loadingError && <p>Loading artifact…</p>}
    {snapshot && !artifact && <p role="alert">Artifact not found on this Object.</p>}
    {artifact && <>
      <header className="artifact-reader-head"><div><p className="artifact-eyebrow">{displayType(artifact)}{document?.key !== null && artifact.id !== document?.latest.id ? " · Historical version" : ""}</p><h1>{artifactLabel(artifact)}</h1></div><div className="artifact-reader-controls">
        {latest && editableText(artifact) && body !== null && !editing && <button className="secondary" type="button" onClick={() => { setDraft(body); setEditBaseRevision(snapshot?.object.revision ?? null); setEditing(true); }}>Edit</button>}
        <details className="artifact-menu"><summary aria-label="Artifact options">⋯</summary><div className="artifact-menu-panel"><button type="button" onClick={() => setPanel(panel === "history" ? null : "history")}>History</button><button type="button" onClick={() => setPanel(panel === "details" ? null : "details")}>Details</button></div></details>
      </div></header>
      {artifact.capture_outcome !== "complete" && <p className="artifact-status">Content {artifact.capture_outcome}{artifact.capture_reason ? `: ${artifact.capture_reason}` : "."}</p>}
      {editing ? <div className="artifact-editor"><label htmlFor="artifact-draft">Research documents</label><textarea id="artifact-draft" value={draft} onChange={(event) => setDraft(event.target.value)} rows={20} /><div className="artifact-actions"><button className="ghost" type="button" disabled={busy} onClick={() => { setEditing(false); setEditBaseRevision(null); setSaveError(null); }}>Cancel</button><button className="primary" type="button" disabled={busy || draft === body || !draft.trim()} onClick={() => void save()}>{busy ? "Saving…" : "Save"}</button></div>{saveError && <p className="form-error" role="alert">{saveError}</p>}</div> : <div className="artifact-body" aria-label="Artifact content">{bodyError ? <p className="form-error" role="alert">{bodyError}</p> : body !== null ? body ? mediaType(artifact) === "text/markdown" ? <Markdown text={body} /> : <pre>{mediaType(artifact) === "application/json" ? prettyJson(body) : body}</pre> : <p className="muted">This artifact is empty.</p> : bodyUnavailable ? <p className="muted">No inline reader is available for this format.</p> : <p>Loading content…</p>}</div>}
      {safeArtifactUri(artifact.uri) && <p><a href={safeArtifactUri(artifact.uri)!} target="_blank" rel="noopener noreferrer">Open original URI ↗</a></p>}
      {panel === "history" && <section id="artifact-history" className="artifact-reader-subsection"><h2>History</h2>{document && document.history.length > 1 ? <ol>{document.history.map((item) => <li key={item.id}><ArtifactLink href={artifactPath(section, objectId, item.id)}>{artifactLabel(item)} · {item.id === document.latest.id ? "Latest" : "Previous version"}</ArtifactLink>{item.created_at && <span className="muted"> · {new Date(item.created_at).toLocaleString()}</span>}</li>)}</ol> : <p className="muted">No earlier versions.</p>}</section>}
      {panel === "details" && <section id="artifact-details" className="artifact-reader-subsection"><h2>Details</h2><dl><dt>Kind</dt><dd>{artifact.kind}</dd><dt>Media type</dt><dd>{artifact.media_type ?? "Unspecified"}</dd><dt>Artifact ID</dt><dd>{artifact.id}</dd><dt>Content hash</dt><dd>{artifact.sha256}</dd><dt>Document key</dt><dd>{document?.key ?? "—"}</dd><dt>Capture</dt><dd>{artifact.capture_outcome}</dd></dl></section>}
    </>}
  </article>;
}

function prettyJson(text: string): string {
  try { return JSON.stringify(JSON.parse(text), null, 2); }
  catch { return text; }
}
