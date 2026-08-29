import { useState } from "react";
import type { ObjectKind, ObjectVisual, TaskStatus, UserAttribution } from "./types";

const typeVisuals: Record<ObjectKind, { icon: string; label: string }> = {
  task: { icon: "✓", label: "Task" },
  chat: { icon: "◌", label: "Chat" },
  user: { icon: "♙", label: "User" },
  entity: { icon: "◎", label: "Entity" },
  memory: { icon: "✦", label: "Memory" },
};

const statusVisuals: Record<TaskStatus, { icon: string; label: string }> = {
  todo: { icon: "○", label: "To do" },
  doing: { icon: "◐", label: "Doing" },
  blocked: { icon: "!", label: "Blocked" },
  review: { icon: "◇", label: "Review" },
  done: { icon: "✓", label: "Done" },
};

export function ObjectTypeBadge({ kind }: { kind: ObjectKind }) {
  const visual = typeVisuals[kind];
  return <span className={`visual-badge type-badge ${kind}`}><span aria-hidden="true">{visual.icon}</span>{visual.label}</span>;
}

export function TaskStatusBadge({ status }: { status: TaskStatus }) {
  const visual = statusVisuals[status];
  return <span className={`visual-badge status-badge ${status}`}><span aria-hidden="true">{visual.icon}</span>{visual.label}</span>;
}

export function StateBadge({ state }: { state: string }) {
  return <span className={`visual-badge state-badge ${state}`}><span aria-hidden="true">●</span>{readable(state)}</span>;
}

export function SourceBadge({ provider }: { provider: string | null | undefined }) {
  if (!provider) return null;
  return <span className={`source-badge ${provider}`} aria-label={`Source: ${readable(provider)}`}>{readable(provider)}</span>;
}

export function AttributionStack({ users, limit = 3 }: { users: UserAttribution[]; limit?: number }) {
  if (users.length === 0) return null;
  const visible = users.slice(0, limit);
  return <span className="attribution-stack" aria-label={users.map((user) => `${user.title}, ${user.role}`).join("; ")}>
    {visible.map((user) => <UserAvatar key={`${user.user_object_id}:${user.role}`} user={user} />)}
    {users.length > visible.length && <span className="avatar-overflow" aria-hidden="true">+{users.length - visible.length}</span>}
  </span>;
}

export function AttributionList({ users }: { users: UserAttribution[] }) {
  if (users.length === 0) return <span className="muted">No attributable User</span>;
  return <span className="attribution-list">{users.map((user) => <span className="attribution-user" key={`${user.user_object_id}:${user.role}`}>
    <UserAvatar user={user} />
    <span><strong>{user.title}</strong><small>{user.role}</small></span>
  </span>)}</span>;
}

export function ObjectContext({ visual }: { visual: ObjectVisual | undefined }) {
  if (!visual || (!visual.source_provider && visual.users.length === 0)) return null;
  return <span className="object-context"><SourceBadge provider={visual.source_provider} /><AttributionStack users={visual.users} /></span>;
}

function UserAvatar({ user }: { user: UserAttribution }) {
  const [failed, setFailed] = useState(false);
  const label = `${user.title}, ${readable(user.user_kind)}, ${user.role}`;
  const style = { "--avatar-hue": avatarHue(user.user_object_id) } as React.CSSProperties;
  return <span className={`user-avatar ${user.user_kind}`} role="img" aria-label={label} title={label} style={style}>
    {user.avatar_url && !failed
      ? <img src={user.avatar_url} alt="" loading="lazy" referrerPolicy="no-referrer" onError={() => setFailed(true)} />
      : <span aria-hidden="true">{initials(user.title)}</span>}
  </span>;
}

function readable(value: string) {
  return value.replaceAll("_", " ").replace(/^./, (character) => character.toUpperCase());
}

function initials(value: string) {
  const parts = value.trim().split(/\s+/).filter(Boolean);
  return (parts.length > 1 ? `${parts[0][0]}${parts.at(-1)?.[0] ?? ""}` : parts[0]?.slice(0, 2) ?? "?").toUpperCase();
}

function avatarHue(id: string) {
  let value = 0;
  for (const character of id) value = (value * 31 + character.charCodeAt(0)) % 360;
  return String(value);
}
