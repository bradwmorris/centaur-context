import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { SchemaWorkspace } from "./SchemaWorkspace";
import type { SchemaSnapshot } from "./types";

vi.mock("./api", () => ({
  api: {
    schema: vi.fn(),
    schemaRows: vi.fn(),
  },
}));

const objects = {
  name: "objects",
  classification: "canonical" as const,
  estimated_row_count: 1,
  columns: [{ name: "id", ordinal: 1, data_type: "uuid", nullable: false, default: null, identity: false, generated: false }],
  constraints: [{ name: "objects_pkey", kind: "primary_key", columns: ["id"], definition: "PRIMARY KEY (id)" }],
};
const tasks = {
  name: "tasks",
  classification: "subtype" as const,
  estimated_row_count: 1,
  columns: [{ name: "object_id", ordinal: 1, data_type: "uuid", nullable: false, default: null, identity: false, generated: false }],
  constraints: [{ name: "tasks_pkey", kind: "primary_key", columns: ["object_id"], definition: "PRIMARY KEY (object_id)" }],
};
const initial: SchemaSnapshot = {
  fingerprint: "first",
  tables: [objects, tasks],
  foreign_keys: [{ name: "tasks_object_fk", source_table: "tasks", source_columns: ["object_id"], target_table: "objects", target_columns: ["id"], one_to_one_subtype: true, nullable: false }],
};
const changed: SchemaSnapshot = { fingerprint: "second", tables: [objects], foreign_keys: [] };

describe("dynamic schema refresh", () => {
  afterEach(() => { cleanup(); vi.clearAllMocks(); });

  it("preserves a valid selection and returns safely to the map when a migration removes it", async () => {
    window.history.replaceState({}, "", "/schema/tasks/structure");
    vi.mocked(api.schema).mockResolvedValueOnce(initial).mockResolvedValue(changed);
    render(<SchemaWorkspace selectedTable="tasks" />);
    expect(await screen.findByRole("heading", { name: "Tasks" })).toBeVisible();
    expect(window.location.pathname).toBe("/schema/tasks/structure");
    const catalog = within(screen.getByRole("navigation", { name: "Schema tables" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Find a table" }), "tasks");
    expect(catalog.getByRole("button", { name: "Tasks 1" })).toBeVisible();
    expect(catalog.queryByRole("button", { name: "Objects 1" })).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Refresh schema" }));
    await waitFor(() => expect(window.location.pathname).toBe("/schema"));
    expect(await screen.findByRole("heading", { name: "Schema map" })).toBeVisible();
    expect(screen.queryByRole("button", { name: /Tasks subtype/ })).not.toBeInTheDocument();
  });

  it("loads an exact focused row from a foreign-key deep link and can return to all rows", async () => {
    window.history.replaceState({}, "", "/schema/tasks/rows?focus_column=object_id&focus_value=object-42");
    vi.mocked(api.schema).mockResolvedValue(initial);
    vi.mocked(api.schemaRows).mockResolvedValue({
      schema_fingerprint: "first",
      table: "tasks",
      rows: [{ object_id: "object-42" }],
      next_cursor: null,
      page_size: 50,
    });
    render(<SchemaWorkspace selectedTable="tasks" />);
    expect(await screen.findByText("object-42")).toBeVisible();
    expect(api.schemaRows).toHaveBeenCalledWith("tasks", undefined, { column: "object_id", value: "object-42" });
    expect(screen.getByText("Focused row")).toBeVisible();
    await userEvent.click(screen.getByRole("button", { name: "Show all rows" }));
    expect(`${window.location.pathname}${window.location.search}`).toBe("/schema/tasks/rows");
  });
});
