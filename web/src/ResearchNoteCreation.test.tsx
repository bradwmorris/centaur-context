import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { NewNote } from "./App";
import { api } from "./api";
import type { Note } from "./types";

vi.mock("./api", async (original) => ({ ...await original<typeof import("./api")>(), api: { createNote: vi.fn() } }));

it("offers only current intents and saves a source-free Idea", async () => {
  vi.mocked(api.createNote).mockResolvedValue({ object_id: "note-idea" } as Note);
  const created = vi.fn();
  render(<NewNote onCancel={vi.fn()} onCreated={created} />);
  const intent = screen.getByLabelText("Intent");
  expect(within(intent).getAllByRole("option").map((option) => option.textContent)).toEqual(["Idea", "Excerpt", "Fact"]);
  fireEvent.change(screen.getByLabelText("Note title"), { target: { value: "Synthetic idea" } });
  fireEvent.change(screen.getByLabelText("Note description"), { target: { value: "A working idea authored by the researcher." } });
  fireEvent.change(screen.getByLabelText("Note content"), { target: { value: "The original thought." } });
  fireEvent.click(screen.getByRole("button", { name: "Create note" }));
  await waitFor(() => expect(created).toHaveBeenCalled());
  expect(api.createNote).toHaveBeenCalledWith(expect.objectContaining({ intent: "idea", content: "The original thought.", derived_from_source_object_ids: [] }));
});
