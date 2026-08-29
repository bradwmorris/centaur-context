import { useState } from "react";
import { connectionPath, interceptNavigation, objectPath } from "./routing";

export function ObjectId({ id, compact = false, copy = true, label = true, rowPill = false }: { id: string; compact?: boolean; copy?: boolean; label?: boolean; rowPill?: boolean }) {
  const [copyState, setCopyState] = useState<"idle" | "copied" | "error">("idle");
  const path = objectPath(id);
  const copyId = async () => {
    try {
      if (!navigator.clipboard?.writeText) throw new Error("Clipboard access is unavailable");
      await navigator.clipboard.writeText(id);
      setCopyState("copied");
      window.setTimeout(() => setCopyState("idle"), 1500);
    } catch {
      setCopyState("error");
    }
  };
  if (rowPill) {
    return <span className="object-identity row-pill">
      <button
        type="button"
        className="object-id-pill"
        data-copy-state={copyState}
        onClick={(event) => { event.stopPropagation(); void copyId(); }}
        aria-label={`Copy Object ID ${id}`}
        title={`Copy full Object ID ${id}`}
      >ID: {id.slice(0, 5)}</button>
      <span className="sr-only" aria-live="polite">{copyState === "copied" ? `Copied Object ID ${id}` : copyState === "error" ? `Could not copy Object ID ${id}` : ""}</span>
    </span>;
  }
  return <span className={compact ? "object-identity compact" : "object-identity"}>
    {label && <span className="identity-label">Object ID</span>}
    <a href={path} onClick={(event) => interceptNavigation(event, path)} title={id} aria-label={`Open Object ID ${id}`}>
      <span aria-hidden="true">{compact ? shortId(id) : id}</span>
    </a>
    {copy && <button type="button" onClick={() => void copyId()} aria-label={`Copy Object ID ${id}`} title="Copy full Object ID">{copyState === "copied" ? "Copied" : copyState === "error" ? "Retry" : "Copy"}</button>}
    <span className="sr-only" aria-live="polite">{copyState === "copied" ? `Copied Object ID ${id}` : copyState === "error" ? `Could not copy Object ID ${id}` : ""}</span>
  </span>;
}

export function ConnectionId({ id, label = true }: { id: string; label?: boolean }) {
  const path = connectionPath(id);
  return <span className="supporting-identity">{label && <span>Connection ID</span>}<a href={path} aria-label={`Open Connection ID ${id}`} onClick={(event) => interceptNavigation(event, path)}>{id}</a></span>;
}

function shortId(value: string) {
  return value.length > 12 ? `${value.slice(0, 8)}…` : value;
}
