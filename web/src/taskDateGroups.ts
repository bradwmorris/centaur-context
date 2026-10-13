import type { Task } from "./types";

export type TaskDateGroup = {
  key: string;
  label: string;
  tasks: Task[];
};

type DateParts = { year: number; month: number; day: number };

/** Groups a task list by its canonical due_at field using local calendar days. */
export function groupListByDateField(
  tasks: Task[],
  field: "due_at",
  now = new Date(),
  timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone,
  showCompleted = false,
): TaskDateGroup[] {
  if (field !== "due_at") return [];

  const today = dateParts(now, timeZone);
  const todayOrdinal = dayOrdinal(today);
  const dated = new Map<number, Task[]>();
  const undated: Task[] = [];
  const completed: Task[] = [];

  for (const task of tasks) {
    if (task.status === "done") {
      if (showCompleted) completed.push(task);
      continue;
    }
    const due = task.due_at ? new Date(task.due_at) : null;
    if (!due || !Number.isFinite(due.getTime())) {
      undated.push(task);
      continue;
    }
    const ordinal = dayOrdinal(dateParts(due, timeZone));
    const bucket = dated.get(ordinal) ?? [];
    bucket.push(task);
    dated.set(ordinal, bucket);
  }

  const groups: TaskDateGroup[] = [];
  const orderedDays = [...dated.keys()].sort((left, right) => left - right);
  for (const ordinal of orderedDays) {
    const bucket = dated.get(ordinal)!;
    bucket.sort(compareDatedTasks);
    const difference = ordinal - todayOrdinal;
    const label = difference < 0 ? "Overdue"
      : difference === 0 ? "Today"
        : difference === 1 ? "Tomorrow"
          : formatDay(partsFromOrdinal(ordinal), today.year);
    const key = difference < 0 ? "overdue" : difference === 0 ? "today" : difference === 1 ? "tomorrow" : `day-${ordinal}`;
    const existing = groups.find((group) => group.key === key);
    if (existing) existing.tasks.push(...bucket);
    else groups.push({ key, label, tasks: bucket });
  }

  if (undated.length) {
    undated.sort(compareStableTasks);
    groups.push({ key: "no-due-date", label: "No due date", tasks: undated });
  }
  if (completed.length) {
    completed.sort(compareCompletedTasks);
    groups.push({ key: "completed", label: "Completed", tasks: completed });
  }
  return groups;
}

export function formatTaskDueDate(value: string | null, now = new Date(), timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone): string {
  if (value === null) return "No due date";
  const due = new Date(value);
  if (!Number.isFinite(due.getTime())) return "Date unavailable";
  const today = dateParts(now, timeZone);
  const date = dateParts(due, timeZone);
  const ordinalDelta = dayOrdinal(date) - dayOrdinal(today);
  const dayLabel = ordinalDelta < 0 ? formatDay(date, today.year)
    : ordinalDelta === 0 ? "Today"
      : ordinalDelta === 1 ? "Tomorrow"
        : formatDay(date, today.year);
  const time = new Intl.DateTimeFormat(undefined, { timeZone, hour: "numeric", minute: "2-digit" }).format(due);
  return `${dayLabel}, ${time}`;
}

function dateParts(value: Date, timeZone: string): DateParts {
  const parts = new Intl.DateTimeFormat("en-US", { timeZone, year: "numeric", month: "numeric", day: "numeric" }).formatToParts(value);
  return {
    year: Number(parts.find((part) => part.type === "year")?.value),
    month: Number(parts.find((part) => part.type === "month")?.value),
    day: Number(parts.find((part) => part.type === "day")?.value),
  };
}

function dayOrdinal({ year, month, day }: DateParts): number {
  return Math.floor(Date.UTC(year, month - 1, day) / 86_400_000);
}

function partsFromOrdinal(ordinal: number): DateParts {
  const date = new Date(ordinal * 86_400_000);
  return { year: date.getUTCFullYear(), month: date.getUTCMonth() + 1, day: date.getUTCDate() };
}

function formatDay(date: DateParts, currentYear: number): string {
  const value = new Date(Date.UTC(date.year, date.month - 1, date.day, 12));
  const parts = new Intl.DateTimeFormat(undefined, {
    timeZone: "UTC",
    weekday: "short",
    day: "numeric",
    month: "short",
    ...(date.year === currentYear ? {} : { year: "numeric" }),
  }).formatToParts(value);
  const weekday = parts.find((part) => part.type === "weekday")?.value ?? "";
  const day = parts.find((part) => part.type === "day")?.value ?? "";
  const month = parts.find((part) => part.type === "month")?.value ?? "";
  const year = parts.find((part) => part.type === "year")?.value;
  return [weekday, day, month, year].filter(Boolean).join(" ");
}

function compareDatedTasks(left: Task, right: Task): number {
  const leftTime = new Date(left.due_at!).getTime();
  const rightTime = new Date(right.due_at!).getTime();
  return leftTime - rightTime || compareStableTasks(left, right);
}

function compareCompletedTasks(left: Task, right: Task): number {
  const leftTime = left.due_at ? new Date(left.due_at).getTime() : Number.NaN;
  const rightTime = right.due_at ? new Date(right.due_at).getTime() : Number.NaN;
  const leftHasDate = Number.isFinite(leftTime);
  const rightHasDate = Number.isFinite(rightTime);
  if (leftHasDate && rightHasDate) return leftTime - rightTime || compareStableTasks(left, right);
  if (leftHasDate) return -1;
  if (rightHasDate) return 1;
  return compareStableTasks(left, right);
}

function compareStableTasks(left: Task, right: Task): number {
  const leftTitle = left.title.toLowerCase();
  const rightTitle = right.title.toLowerCase();
  return compareText(leftTitle, rightTitle) || compareText(left.title, right.title) || compareText(left.object_id, right.object_id);
}

function compareText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}
