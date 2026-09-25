import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

/** Specialized views contribute controls to the same host toolbar. */
export function WorkspaceToolbar({ children, className }: { children: ReactNode; className: string }) {
  const [target, setTarget] = useState<HTMLElement | null>(null);
  useEffect(() => { setTarget(document.getElementById("workspace-toolbar-slot")); }, []);
  const content = <div className={className}>{children}</div>;
  return target ? createPortal(content, target) : content;
}
