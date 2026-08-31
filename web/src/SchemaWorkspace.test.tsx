import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { SchemaWorkspace } from "./SchemaWorkspace";
import type { SchemaSnapshot } from "./types";

vi.mock("./api", () => ({
  api: {
    schema: vi.fn(),
    schemaRows: vi.fn(),
    schemaProfile: vi.fn(),
  },
}));

const objects = {
  name: "objects",
  classification: "canonical" as const,
  estimated_row_count: 1,
  columns: [{ name: "id", ordinal: 1, data_type: "uuid", nullable: false, default: null, identity: false, generated: false }],
  constraints: [{ name: "objects_pkey", kind: "primary_key", columns: ["id"], definition: "PRIMARY KEY (id)" }],
  indexes: [], triggers: [],
};
const tasks = {
  name: "tasks",
  classification: "subtype" as const,
  estimated_row_count: 1,
  columns: [{ name: "object_id", ordinal: 1, data_type: "uuid", nullable: false, default: null, identity: false, generated: false }],
  constraints: [{ name: "tasks_pkey", kind: "primary_key", columns: ["object_id"], definition: "PRIMARY KEY (object_id)" }],
  indexes: [{ name: "tasks_pkey", unique: true, primary: true, constraint_backed: true, definition: "CREATE UNIQUE INDEX tasks_pkey ON tasks (object_id)" }],
  triggers: [{ name: "tasks_prevent_removal", enabled: "enabled", definition: "CREATE TRIGGER tasks_prevent_removal" }],
};
const initial: SchemaSnapshot = {
  fingerprint: "first",
  tables: [objects, tasks],
  foreign_keys: [{ name: "tasks_object_fk", source_table: "tasks", source_columns: ["object_id"], target_table: "objects", target_columns: ["id"], one_to_one_subtype: true, nullable: false }],
};
const changed: SchemaSnapshot = { fingerprint: "second", tables: [objects], foreign_keys: [] };

describe("dynamic schema refresh", () => {
  afterEach(() => { cleanup(); vi.clearAllMocks(); });

  it("shows table rows and returns safely to the map when a migration removes the table", async () => {
    window.history.replaceState({}, "", "/schema/tasks/rows");
    vi.mocked(api.schema).mockResolvedValueOnce(initial).mockResolvedValue(changed);
    vi.mocked(api.schemaRows).mockResolvedValue({ schema_fingerprint: "first", table: "tasks", rows: [], next_cursor: null, page_size: 50 });
    render(<SchemaWorkspace selectedTable="tasks" />);
    expect(await screen.findByRole("heading", { name: "Tasks" })).toBeVisible();
    expect(window.location.pathname).toBe("/schema/tasks/rows");
    expect(screen.getByRole("link", { name: "← Schema map" })).toHaveAttribute("href", "/schema");
    expect(screen.queryByRole("navigation", { name: "Schema tables" })).not.toBeInTheDocument();

    window.dispatchEvent(new Event("focus"));
    await waitFor(() => expect(window.location.pathname).toBe("/schema"));
    expect(screen.queryByRole("heading", { name: "Schema map" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Tasks subtype/ })).not.toBeInTheDocument();
  });

  it("opens a table's rows directly from the schema map", async () => {
    window.history.replaceState({}, "", "/schema");
    vi.mocked(api.schema).mockResolvedValue(initial);
    render(<SchemaWorkspace selectedTable={null} />);

    const taskNode = await screen.findByRole("button", { name: /Tasks subtype/ });
    expect(screen.queryByRole("list", { name: "Schema relationships" })).not.toBeInTheDocument();
    await userEvent.click(taskNode);

    expect(window.location.pathname).toBe("/schema/tasks/rows");
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
