import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AttributionList, AttributionStack, ObjectTypeBadge, SourceBadge, TaskStatusBadge } from "./RecordVisuals";

const user = {
  object_id: "11111111-1111-4111-8111-111111111111",
  user_object_id: "11111111-1111-4111-8111-111111111111",
  title: "Example Human",
  user_kind: "human" as const,
  role: "source author" as const,
  avatar_url: "https://example.test/avatar.png",
  avatar_asset_url: "/api/v2/identity-assets/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/example.png",
};

describe("record visual language", () => {
  it("gives every Object type and Task state text plus a non-colour icon", () => {
    render(<>
      {(["task", "chat", "user", "entity", "memory"] as const).map((kind) => <ObjectTypeBadge kind={kind} key={kind} />)}
      {(["todo", "doing", "blocked", "review", "done"] as const).map((status) => <TaskStatusBadge status={status} key={status} />)}
    </>);
    for (const label of ["Task", "Chat", "User", "Entity", "Memory", "To do", "Doing", "Blocked", "Review", "Done"]) {
      expect(screen.getByText(label)).toBeVisible();
    }
    expect(screen.getByText("Memory")).toHaveTextContent("✦Memory");
    expect(screen.getByText("Blocked")).toHaveTextContent("!Blocked");
  });

  it("uses an accessible Slack source icon", () => {
    const { container } = render(<SourceBadge provider="slack" />);
    expect(screen.getByLabelText("Source: Slack")).toBeVisible();
    expect(screen.getByLabelText("Source: Slack")).toHaveTextContent("");
    expect(container.querySelector("svg")).toBeVisible();
  });

  it("falls back deterministically when an avatar image is broken", () => {
    const { container } = render(<AttributionStack users={[user]} />);
    fireEvent.error(container.querySelector("img")!);
    expect(screen.getByRole("img", { name: "Example Human, Human, source author" })).toHaveTextContent("EH");
  });

  it("renders only the same-origin asset route and leaves provider URLs inactive", () => {
    const { container } = render(<AttributionStack users={[user]} />);
    expect(container.querySelector("img")).toHaveAttribute("src", user.avatar_asset_url);
    expect(container.querySelector("img")).not.toHaveAttribute("src", user.avatar_url);
  });

  it("shows the evidence-backed attribution role in detail contexts", () => {
    render(<AttributionList users={[user]} />);
    expect(screen.getByText("Example Human")).toBeVisible();
    expect(screen.getByText("source author")).toBeVisible();
  });
});
