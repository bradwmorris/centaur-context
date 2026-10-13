import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { api } from "./api";
import { EventEntityDetail, type EntityCategory, type SubtypeFields } from "./EventEntityFields";

vi.mock("./api", () => ({ api: { readSubtype: vi.fn(), entityCategories: vi.fn(), applyCorrections: vi.fn() } }));
const category = (id: string, label: string): EntityCategory => ({ id, label, slug: id, definition: `${label} definition`, aliases: [], legacy_kind: "other", revision: 1, archived_at: null });
const categories = [category("org", "Organization"), category("fund", "Fund"), category("person", "Person")];
const props = { id: "first", kind: "entity", revision: 1, onChanged: vi.fn() };
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.entityCategories).mockResolvedValue({ categories, revision: "1" });
});

it("shows only assigned labels once, primary first, with no mutation controls", async () => {
  vi.mocked(api.readSubtype).mockResolvedValue({ category_ids: ["fund", "org", "fund"], primary_category_id: "org" });
  render(<EventEntityDetail {...props} />);
  await screen.findByText("Organization");
  expect(screen.getAllByRole("listitem").map(item => item.textContent)).toEqual(["Organization", "Fund"]);
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
  expect(screen.getByRole("status")).toHaveTextContent("Loading classifications");
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
  await screen.findByText("No classification assigned.");
  vi.mocked(api.readSubtype).mockResolvedValue({ category_ids: ["missing"] });
  rerender(<EventEntityDetail {...props} revision={2} />);
  await screen.findByText("Classification unavailable");
  vi.mocked(api.entityCategories).mockRejectedValue(new Error("Catalogue unavailable"));
  rerender(<EventEntityDetail {...props} revision={3} />);
  await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("Could not load classifications"));
  expect(screen.queryByText("No classification assigned.")).toBeNull();
  expect(screen.queryByRole("listitem")).toBeNull();
});

it("retains Event occurrence editing", async () => {
  vi.mocked(api.readSubtype).mockResolvedValue({ starts_at: "2026-10-13", starts_at_precision: "day" });
  vi.mocked(api.applyCorrections).mockResolvedValue({});
  render(<EventEntityDetail {...props} kind="event" />);
  const start = await screen.findByRole("textbox", { name: "Event start" });
  fireEvent.change(start, { target: { value: "2026-10-14" } });
  fireEvent.click(screen.getByRole("button", { name: "Save properties" }));
  await waitFor(() => expect(api.applyCorrections).toHaveBeenCalledWith([expect.objectContaining({
    object_id: "first", expected_revision: 1, changes: expect.objectContaining({ starts_at: "2026-10-14", starts_at_precision: "day" }),
  })]));
  expect(api.entityCategories).not.toHaveBeenCalled();
});
