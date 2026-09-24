import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TaskAssignee, TaskIssueLink, TaskReadiness, TaskProject } from "./TaskIdentity";
import type { Task, UserAttribution } from "./types";

const owner: UserAttribution = { object_id: "task", user_object_id: "owner", title: "Assigned Person", user_kind: "human", role: "owner", avatar_url: null, avatar_asset_url: "/avatar.png" };
const participant = { ...owner, user_object_id: "participant", title: "Creator Person", role: "participant" as const };
const task = { object_id: "task", owner_object_id: "owner", due_at: null, brief_markdown: null } as Task;

it("shows only the explicitly assigned user, with image failure fallback", () => {
  const { container } = render(<TaskAssignee task={task} visuals={new Map([["task", { object_id: "task", source_provider: null, users: [participant, owner] }]])} showName />);
  expect(screen.getByText("Assigned Person")).toBeInTheDocument();
  expect(screen.queryByText("Creator Person")).not.toBeInTheDocument();
  fireEvent.error(container.querySelector("img")!);
  expect(container.querySelector("img")).toBeNull();
  expect(screen.getByRole("img")).toHaveAccessibleName(/Assigned Person/);
});

it("never substitutes an attributed participant for a missing assignee", () => {
  const visuals = new Map([["task", { object_id: "task", source_provider: null, users: [participant] }]]);
  const { rerender } = render(<TaskAssignee task={{ ...task, owner_object_id: null }} visuals={visuals} />);
  expect(screen.getByText("Unassigned")).toBeInTheDocument();
  rerender(<TaskAssignee task={task} visuals={visuals} />);
  expect(screen.getByText("Assigned user unavailable")).toBeInTheDocument();
  expect(screen.queryByRole("img")).not.toBeInTheDocument();
});

it("opens the canonical GitHub issue independently of row navigation", () => {
  const openRow = vi.fn();
  render(<div onClick={openRow}><TaskIssueLink url="https://github.com/example/project/issues/42" /></div>);
  const link = screen.getByRole("link", { name: "Open GitHub issue example/project/issues/42" });
  expect(link).toHaveAttribute("href", "https://github.com/example/project/issues/42");
  expect(link).toHaveAttribute("rel", "noopener noreferrer");
  fireEvent.click(link);
  expect(openRow).not.toHaveBeenCalled();
});

describe("legacy rendering", () => {
  it.each([null, "javascript:alert(1)", "https://github.com.evil.test/a/b/issues/1", "https://github.com/a/b/pull/1"])("does not link unsafe or non-issue URL %s", (url) => {
    render(<TaskIssueLink url={url} />);
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  });
  it("explains missing execution information without hiding the task", () => {
    render(<TaskReadiness task={{ ...task, owner_object_id: null, work_kind: "code", github_issue_url: null }} />);
    expect(screen.getByText("Needs assignee, due date, brief, GitHub issue")).toBeInTheDocument();
  });
});

it("shows General as a project and explicitly identifies missing assignments", () => {
  const { rerender } = render(<TaskProject task={{ ...task, brief_markdown: "Project: general" }} />);
  expect(screen.getByLabelText("Project: General")).toBeTruthy();
  rerender(<TaskProject task={{ ...task, brief_markdown: null }} />);
  expect(screen.getByLabelText("Project: No project")).toBeTruthy();
});
