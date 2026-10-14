import { useEffect, useState } from "react";
import { api } from "./api";
import { objectDetailPath } from "./routing";
import type { AppRoute } from "./routing";

export function CanonicalObjectRoute({ id, artifact, children }: {
  id: string;
  artifact: AppRoute["artifact"];
  children: React.ReactNode;
}) {
  const [resolved, setResolved] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const artifactKind = artifact?.kind ?? "";
  const artifactId = artifact?.id ?? "";
  useEffect(() => {
    let active = true;
    setResolved(null);
    setError(null);
    void api.object(id).then((object) => {
      if (!active) return;
      if (object.id !== id || typeof object.kind !== "string") throw new Error("The server returned a different Object.");
      if (artifact && object.kind !== "source" && object.kind !== "note") throw new Error("This Object does not support artifact documents.");
      const target = objectDetailPath(object.kind, id, artifact);
      const current = `${window.location.pathname}${window.location.search}${window.location.hash}`;
      const next = `${target}${window.location.search}${window.location.hash}`;
      if (current !== next) {
        window.history.replaceState(window.history.state, "", next);
        window.dispatchEvent(new PopStateEvent("popstate"));
      }
      setResolved(id);
    }).catch((cause) => {
      if (active) setError(cause instanceof Error ? cause.message : String(cause));
    });
    return () => { active = false; };
  }, [id, artifactKind, artifactId]);

  if (error) return <div className="blank-state"><span>◇</span><h2>Could not load</h2><p>{error}</p></div>;
  if (resolved !== id) return <div className="blank-state" role="status"><span>◇</span><h2>Loading</h2><p>Reading the shared record…</p></div>;
  return children;
}
