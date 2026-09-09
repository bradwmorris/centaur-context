import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { ApiError } from "./api";

type InlineEditorProps = {
  label: string;
  value: string;
  onSave: (value: string) => Promise<string | void>;
  onReload?: () => Promise<void>;
  multiline?: boolean;
  required?: boolean;
  maxLength?: number;
  placeholder?: string;
  className?: string;
  heading?: boolean;
};

export function InlineEditor({ label, value, onSave, onReload, multiline = false, required = false, maxLength, placeholder, className = "", heading = false }: InlineEditorProps) {
  const [editing, setEditing] = useState(false);
  const [confirmed, setConfirmed] = useState(value);
  const [draft, setDraft] = useState(value);
  const [state, setState] = useState<"idle" | "dirty" | "saving" | "saved" | "error" | "conflict">("idle");
  const [error, setError] = useState("");
  const [editorMinHeight, setEditorMinHeight] = useState(0);
  const confirmedRef = useRef(value);
  const draftRef = useRef(value);
  const savingRef = useRef(false);
  const queuedRef = useRef<string | null>(null);
  const inputRef = useRef<HTMLInputElement | HTMLTextAreaElement>(null);

  useEffect(() => {
    if (draftRef.current !== confirmedRef.current || savingRef.current) return;
    confirmedRef.current = value;
    draftRef.current = value;
    setConfirmed(value);
    setDraft(value);
  }, [value]);

  useEffect(() => {
    if (editing) inputRef.current?.focus();
  }, [editing]);

  useLayoutEffect(() => {
    if (!editing || !multiline) return;
    const input = inputRef.current;
    if (!(input instanceof HTMLTextAreaElement)) return;
    input.style.height = "auto";
    input.style.height = `${Math.max(editorMinHeight, input.scrollHeight)}px`;
  }, [draft, editing, editorMinHeight, multiline]);

  const commit = async (candidate = draftRef.current) => {
    const next = candidate;
    if (required && !next.trim()) {
      setState("error");
      setError(`${label} cannot be empty.`);
      return;
    }
    if (next === confirmedRef.current) {
      setState("idle");
      setError("");
      return;
    }
    if (savingRef.current) {
      queuedRef.current = next;
      return;
    }
    savingRef.current = true;
    let succeeded = false;
    setState("saving");
    setError("");
    try {
      const canonical = (await onSave(next)) ?? next;
      succeeded = true;
      confirmedRef.current = canonical;
      setConfirmed(canonical);
      if (draftRef.current === next) {
        draftRef.current = canonical;
        setDraft(canonical);
        setState("saved");
        window.setTimeout(() => setState((current) => current === "saved" ? "idle" : current), 1200);
      } else {
        queuedRef.current = draftRef.current;
        setState("dirty");
      }
    } catch (cause) {
      const conflict = cause instanceof ApiError && cause.status === 409;
      setState(conflict ? "conflict" : "error");
      setError(conflict ? "Server data changed. Your draft is preserved; reload or retry after reviewing it." : cause instanceof Error ? cause.message : "Could not save. Your draft is preserved.");
    } finally {
      savingRef.current = false;
      const queued = queuedRef.current;
      queuedRef.current = null;
      if (succeeded && queued !== null && queued !== confirmedRef.current) void commit(queued);
    }
  };

  useEffect(() => {
    if (!editing || state !== "dirty") return;
    const timeout = window.setTimeout(() => void commit(), 800);
    return () => window.clearTimeout(timeout);
  }, [draft, editing, state]);

  const change = (next: string) => {
    draftRef.current = next;
    setDraft(next);
    setState(next === confirmedRef.current ? "idle" : "dirty");
    setError("");
  };
  const cancel = () => {
    queuedRef.current = null;
    draftRef.current = confirmedRef.current;
    setDraft(confirmedRef.current);
    setState("idle");
    setError("");
    setEditing(false);
  };
  const reload = async () => {
    cancel();
    await onReload?.();
  };
  const keyDown = (event: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    if (event.key === "Escape") { event.preventDefault(); cancel(); return; }
    if ((!multiline && event.key === "Enter") || (multiline && event.key === "Enter" && (event.metaKey || event.ctrlKey))) {
      event.preventDefault();
      void commit();
    }
  };
  const classes = `inline-editor ${multiline ? "multiline" : "singleline"} ${className}`.trim();

  if (!editing) return <div className={classes} role={heading ? "heading" : undefined} aria-level={heading ? 1 : undefined}>
    <button type="button" className="inline-editor-view" onClick={(event) => {
      if (multiline) setEditorMinHeight(Math.ceil(event.currentTarget.getBoundingClientRect().height));
      setEditing(true);
    }} aria-label={`Edit ${label}`}>
      <span>{confirmed || placeholder || `Add ${label}`}</span>
    </button>
    <span className="sr-only" role="status" aria-live="polite">{state === "saved" ? `${label} saved` : ""}</span>
  </div>;

  const common = { ref: inputRef as never, value: draft, onChange: (event: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => change(event.target.value), onKeyDown: keyDown, onBlur: () => void commit(), "aria-label": label, maxLength, placeholder };
  return <div className={classes} data-state={state} role={heading ? "heading" : undefined} aria-level={heading ? 1 : undefined}>
    <div className="inline-editor-control">
      {multiline ? <textarea {...common} rows={1} style={{ minHeight: editorMinHeight || undefined }} /> : <input {...common} />}
      <button type="button" className="inline-editor-save" onMouseDown={(event) => event.preventDefault()} onClick={() => void commit()} aria-label={`Save ${label}`} disabled={state === "saving"}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 2.5h8.5L13.5 4v9.5h-11v-11Z"/><path d="M5 2.5v4h5v-4M5 13.5V9h6v4.5"/></svg>
      </button>
    </div>
    <span className="inline-editor-status" role="status" aria-live="polite">{state === "saving" ? "Saving…" : state === "saved" ? "Saved" : error}</span>
    {(state === "error" || state === "conflict") && <span className="inline-editor-actions"><button type="button" onMouseDown={(event) => event.preventDefault()} onClick={() => void commit()}>Retry</button>{state === "conflict" && onReload && <button type="button" onMouseDown={(event) => event.preventDefault()} onClick={() => void reload()}>Reload server value</button>}</span>}
  </div>;
}
