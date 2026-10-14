import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { api } from "./api";
import { EventEntityDetail, type EntityCategory, type SubtypeFields } from "./EventEntityFields";

vi.mock("./api", () => ({ api: { readSubtype: vi.fn(), entityCategories: vi.fn(), applyCorrections: vi.fn() } }));
const category = (id: string, label: string): EntityCategory => ({ id, label, slug: id, definition: `${label} definition`, aliases: [], legacy_kind: "other", revision: 1, archived_at: null });
const categories = [category("org", "Organization"), category("fund", "Fund"), category("person", "Person")];
const props = { id: "first", kind: "entity", revision: 1 };
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.entityCategories).mockResolvedValue({ categories, revision: "1" });
});

it("shows only the primary kind label with no mutation controls", async () => {
  vi.mocked(api.readSubtype).mockResolvedValue({ category_ids: ["fund", "org", "fund"], primary_category_id: "org" });
  render(<EventEntityDetail {...props} />);
  await screen.findByText("Organization");
  expect(screen.getByRole("group", { name: "Entity kind" })).toHaveTextContent(/^Organization$/);
  expect(screen.queryByText("Fund")).toBeNull();
  expect(screen.queryByText("Person")).toBeNull();
  expect(screen.queryByText(/definition/)).toBeNull();
  expect(screen.queryByRole("checkbox")).toBeNull();
  expect(screen.queryByRole("combobox")).toBeNull();
  expect(screen.queryByRole("button")).toBeNull();
  expect(api.applyCorrections).not.toHaveBeenCalled();
});

it("clears old labels on navigation and ignores an obsolete response", async () => {
  let resolveFirst!: (fields: SubtypeFields) => void;
  vi.mocked(api.readSubtype).mockImplementation(id => id === "first" ? new Promise(resolve => { resolveFirst = resolve; }) : Promise.resolve({ category_ids: ["person"], primary_category_id: "person" }));
  const { rerender } = render(<EventEntityDetail {...props} />);
  expect(screen.getByRole("status")).toHaveTextContent("Loading kind");
  rerender(<EventEntityDetail {...props} id="second" />);
  await screen.findByText("Person");
  await act(async () => resolveFirst({ category_ids: ["org"], primary_category_id: "org" }));
  expect(screen.queryByText("Organization")).toBeNull();
  vi.mocked(api.readSubtype).mockReturnValue(new Promise(() => {}));
  rerender(<EventEntityDetail {...props} id="third" />);
  expect(screen.queryByText("Person")).toBeNull();
  expect(screen.getByRole("status")).toBeVisible();
});

it("distinguishes no assignment, unavailable labels and failed reads", async () => {
  vi.mocked(api.readSubtype).mockResolvedValue({ category_ids: [] });
  const { rerender } = render(<EventEntityDetail {...props} />);
  await screen.findByText("Kind not recorded");
  vi.mocked(api.readSubtype).mockResolvedValue({ category_ids: ["missing"] });
  rerender(<EventEntityDetail {...props} revision={2} />);
  await screen.findByText("Kind unavailable");
  vi.mocked(api.entityCategories).mockRejectedValue(new Error("Catalogue unavailable"));
  rerender(<EventEntityDetail {...props} revision={3} />);
  await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("Could not load kind"));
  expect(screen.queryByText("Kind not recorded")).toBeNull();
  expect(screen.queryByRole("listitem")).toBeNull();
});

it.each([
  ["year", "2026", "2027"],
  ["month", "2026-10", "2026-12"],
  ["day", "2026-10-13", "2026-10-14"],
  ["instant", "2026-10-13T09:30:00+11:00", "2026-10-13T10:00:00+11:00"],
])("shows %s timing read-only without inventing precision or changing timezone", async (precision, start, end) => {
  vi.mocked(api.readSubtype).mockResolvedValue({ starts_at: start, starts_at_precision: precision, ends_at: end, ends_at_precision: precision, timezone: "Australia/Sydney" });
  render(<EventEntityDetail {...props} kind="event" />);
  expect(await screen.findByText(start)).toHaveAttribute("datetime", start);
  expect(screen.getByText(end)).toHaveAttribute("datetime", end);
  expect(screen.getByText("Australia/Sydney")).toBeVisible();
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  expect(screen.queryByRole("button")).not.toBeInTheDocument();
  expect(screen.queryByText("Event timing")).not.toBeInTheDocument();
  expect(api.applyCorrections).not.toHaveBeenCalled();
  expect(api.entityCategories).not.toHaveBeenCalled();
});

it("keeps partial, unknown and failed Event timing distinct", async () => {
  vi.mocked(api.readSubtype).mockResolvedValue({ ends_at: "2026-10", ends_at_precision: "month" });
  const { rerender } = render(<EventEntityDetail {...props} kind="event" />);
  expect(await screen.findByText("2026-10")).toBeVisible();
  expect(screen.queryByText("Start")).not.toBeInTheDocument();
  vi.mocked(api.readSubtype).mockResolvedValue({});
  rerender(<EventEntityDetail {...props} kind="event" revision={2} />);
  expect(await screen.findByText("Timing not recorded.")).toBeVisible();
  vi.mocked(api.readSubtype).mockRejectedValue(new Error("Synthetic failure"));
  rerender(<EventEntityDetail {...props} kind="event" revision={3} />);
  expect(await screen.findByRole("alert")).toHaveTextContent("Could not load occurrence");
  expect(screen.queryByText("Timing not recorded.")).not.toBeInTheDocument();
});
