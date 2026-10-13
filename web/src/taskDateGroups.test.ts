import { describe, expect, it } from "vitest";
import type { Task } from "./types";
import { formatTaskDueDate, groupListByDateField } from "./taskDateGroups";

const task = (object_id: string, due_at: string | null, title = object_id, status: Task["status"] = "todo"): Task => ({
  object_id,
  title,
  description: "Synthetic task fixture",
  lifecycle: "active",
  revision: 1,
  provenance: {},
  protected: true,
  status,
  priority: "medium",
  owner_object_id: null,
  agent_suitable: false,
  blocked_reason: null,
  due_at,
  completed_at: status === "done" ? "2026-10-12T10:00:00Z" : null,
  github_issue_url: null,
  brief_markdown: null,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
});

describe("task due-date grouping", () => {
  const sydneyNow = new Date("2026-10-13T02:00:00Z"); // 13 Oct, 13:00 in Sydney

  it("groups calendar days, keeps same-day past times in Today, and sorts each section stably", () => {
    const groups = groupListByDateField([
      task("tomorrow-later", "2026-10-14T02:00:00Z", "Later"),
      task("today-late", "2026-10-13T01:00:00Z", "Today later"),
      task("today-early", "2026-10-12T14:00:00Z", "Today earlier"),
      task("overdue", "2026-10-12T01:00:00Z"),
      task("day-15", "2026-10-15T01:00:00Z"),
      task("null", null),
      task("invalid", "legacy-date"),
    ], "due_at", sydneyNow, "Australia/Sydney");

    expect(groups.map(({ key, label }) => [key, label])).toEqual([
      ["overdue", "Overdue"], ["today", "Today"], ["tomorrow", "Tomorrow"], ["day-20741", "Thu 15 Oct"], ["no-due-date", "No due date"],
    ]);
    expect(groups[1].tasks.map(({ object_id }) => object_id)).toEqual(["today-early", "today-late"]);
    expect(groups.at(-1)?.tasks.map(({ object_id }) => object_id)).toEqual(["invalid", "null"]);
  });

  it("keeps completed tasks out of overdue and shows them only in the final group when enabled", () => {
    const items = [
      task("done-new", "2026-10-13T00:00:00Z", "Done later", "done"),
      task("done-old", "2026-10-11T00:00:00Z", "Done", "done"),
      task("done-undated", null, "Undated completed", "done"),
      task("open-old", "2026-10-11T00:00:00Z"),
    ];
    expect(groupListByDateField(items, "due_at", sydneyNow, "Australia/Sydney").map(({ key }) => key)).toEqual(["overdue"]);
    const groups = groupListByDateField(items, "due_at", sydneyNow, "Australia/Sydney", true);
    expect(groups.map(({ key }) => key)).toEqual(["overdue", "completed"]);
    expect(groups[0].tasks.map(({ object_id }) => object_id)).toEqual(["open-old"]);
    expect(groups[1].tasks.map(({ object_id }) => object_id)).toEqual(["done-old", "done-new", "done-undated"]);
  });

  it("uses the selected local timezone near UTC midnight", () => {
    const current = new Date("2026-10-13T10:00:00Z");
    const due = task("utc-edge", "2026-10-13T23:30:00Z");
    expect(groupListByDateField([due], "due_at", current, "Australia/Sydney")[0].label).toBe("Tomorrow");
    expect(groupListByDateField([due], "due_at", current, "America/Los_Angeles")[0].label).toBe("Today");
  });

  it("uses calendar-day boundaries across the Sydney DST transition", () => {
    const beforeTransition = new Date("2026-10-03T13:30:00Z"); // 23:30 before clocks advance
    const nextMorning = task("dst-next-day", "2026-10-03T22:00:00Z"); // 09:00 after clocks advance
    expect(groupListByDateField([nextMorning], "due_at", beforeTransition, "Australia/Sydney")[0].label).toBe("Tomorrow");
  });

  it("includes the year when a later date crosses into another year", () => {
    const groups = groupListByDateField([task("jan-3", "2027-01-03T12:00:00Z")], "due_at", new Date("2026-12-31T12:00:00Z"), "UTC");
    expect(groups[0].label).toBe("Sun 3 Jan 2027");
  });

  it("formats visible row deadlines and makes malformed dates explicit", () => {
    expect(formatTaskDueDate(null, sydneyNow, "Australia/Sydney")).toBe("No due date");
    expect(formatTaskDueDate("legacy-date", sydneyNow, "Australia/Sydney")).toBe("Date unavailable");
    expect(formatTaskDueDate("2026-10-15T00:00:00Z", sydneyNow, "Australia/Sydney")).toMatch(/^Thu 15 Oct, /);
  });
});
