import { useState } from "react";
import type { ObjectKind, ObjectVisual, TaskStatus, UserAttribution } from "./types";

const typeVisuals: Record<ObjectKind, { icon: string; label: string }> = {
  task: { icon: "✓", label: "Task" },
  chat: { icon: "◌", label: "Chat" },
  user: { icon: "♙", label: "User" },
  entity: { icon: "◎", label: "Entity" },
  memory: { icon: "✦", label: "Memory" },
  source: { icon: "▤", label: "Source" },
  note: { icon: "▱", label: "Note" },
  theme: { icon: "#", label: "Theme" },
  external_action: { icon: "↗", label: "External action" },
};

const statusVisuals: Record<TaskStatus, { icon: string; label: string }> = {
  backlog: { icon: "◌", label: "Backlog" },
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
  const label = readable(provider);
  return <span className={`source-badge ${provider}`} aria-label={`Source: ${label}`} title={`Source: ${label}`}>
    {provider === "slack" ? <SlackIcon /> : label}
  </span>;
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
    {user.avatar_asset_url && !failed
      ? <img src={user.avatar_asset_url} alt="" loading="lazy" referrerPolicy="no-referrer" onError={() => setFailed(true)} />
      : <span aria-hidden="true">{initials(user.title)}</span>}
  </span>;
}

function SlackIcon() {
  return <svg aria-hidden="true" viewBox="0 0 256 256">
    <path fill="#e01e5a" d="M53.841 161.32c0 14.832-11.987 26.82-26.819 26.82S.203 176.152.203 161.32c0-14.831 11.987-26.818 26.82-26.818H53.84zm13.41 0c0-14.831 11.987-26.818 26.819-26.818s26.819 11.987 26.819 26.819v67.047c0 14.832-11.987 26.82-26.82 26.82c-14.83 0-26.818-11.988-26.818-26.82z" />
    <path fill="#36c5f0" d="M94.07 53.638c-14.832 0-26.82-11.987-26.82-26.819S79.239 0 94.07 0s26.819 11.987 26.819 26.819v26.82zm0 13.613c14.832 0 26.819 11.987 26.819 26.819s-11.987 26.819-26.82 26.819H26.82C11.987 120.889 0 108.902 0 94.069c0-14.83 11.987-26.818 26.819-26.818z" />
    <path fill="#2eb67d" d="M201.55 94.07c0-14.832 11.987-26.82 26.818-26.82s26.82 11.988 26.82 26.82s-11.988 26.819-26.82 26.819H201.55zm-13.41 0c0 14.832-11.988 26.819-26.82 26.819c-14.831 0-26.818-11.987-26.818-26.82V26.82C134.502 11.987 146.489 0 161.32 0s26.819 11.987 26.819 26.819z" />
    <path fill="#ecb22e" d="M161.32 201.55c14.832 0 26.82 11.987 26.82 26.818s-11.988 26.82-26.82 26.82c-14.831 0-26.818-11.988-26.818-26.82V201.55zm0-13.41c-14.831 0-26.818-11.988-26.818-26.82c0-14.831 11.987-26.818 26.819-26.818h67.25c14.832 0 26.82 11.987 26.82 26.819s-11.988 26.819-26.82 26.819z" />
  </svg>;
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
