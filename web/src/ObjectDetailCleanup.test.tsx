import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import App from "./App";
import { createObjectFixtureApi } from "./test-fixtures/objectApi";

beforeEach(() => { vi.stubGlobal("fetch", vi.fn(createObjectFixtureApi())); });

it("shows Source artifacts and connected Notes once without creation or correction prompts", async () => {
  window.history.replaceState({}, "", "/sources/synthetic-source");
  render(<App />);
  expect(await screen.findByRole("link", { name: "Synthetic note" })).toBeVisible();
  expect(screen.getAllByRole("link", { name: "Synthetic note" })).toHaveLength(1);
  expect(screen.queryByRole("heading", { name: "Notes" })).not.toBeInTheDocument();
  expect(screen.queryByText("No atomic notes derive from this Source.")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /Add artifact|Add research document|Review or correct evidence/ })).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: /Research document.*2 versions/ })).toHaveAttribute("href", "/sources/synthetic-source/working/research");
  expect(screen.getByRole("link", { name: /Original transcript.*unavailable/ })).toBeVisible();
  expect(vi.mocked(fetch).mock.calls.some(([url]) => String(url).startsWith("/api/v2/notes"))).toBe(false);
  expect(vi.mocked(fetch).mock.calls.some(([url]) => String(url).includes("connection-graph"))).toBe(false);
  fireEvent.click(screen.getByRole("link", { name: "Synthetic note" }));
  expect(await screen.findByRole("button", { name: "Edit Note content" })).toHaveTextContent("Synthetic complete Note body.");
});

it("keeps Task workflow editing, secondary information and relationship creation reachable", async () => {
  window.history.replaceState({}, "", "/tasks/synthetic-task");
  render(<App />);
  expect(await screen.findByRole("button", { name: "Edit Task brief" })).toBeVisible();
  expect(screen.queryByRole("button", { name: "Make a Routine" })).not.toBeInTheDocument();
  expect(screen.queryByRole("region", { name: "Routine schedule" })).not.toBeInTheDocument();
  fireEvent.change(screen.getByRole("combobox", { name: "Task status" }), { target: { value: "doing" } });
  await waitFor(() => expect(fetch).toHaveBeenCalledWith("/api/v2/tasks/synthetic-task", expect.objectContaining({ method: "PATCH", body: expect.stringContaining('"status":"doing"') })));
  fireEvent.click(screen.getByText("More details", { selector: "summary" }));
  expect(screen.getByRole("button", { name: "Edit GitHub issue URL" })).toBeVisible();
  expect(screen.getByRole("checkbox", { name: "Suitable" })).toBeVisible();
  expect(screen.getByText("Not complete")).toBeVisible();
  fireEvent.click(screen.getByText("Connect an Object", { selector: "summary" }));
  expect(await screen.findByRole("combobox", { name: "Connection kind" })).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(screen.queryByRole("combobox", { name: "Connection kind" })).not.toBeInTheDocument());
});

it("retains the complete long related title and its native destination", async () => {
  vi.stubGlobal("fetch", vi.fn(createObjectFixtureApi({ longTitles: true })));
  window.history.replaceState({}, "", "/sources/synthetic-source");
  render(<App />);
  const title = await screen.findByRole("link", { name: /^Synthetic research note:/ });
  expect(title).toHaveTextContent("Synthetic research note: preserving complete related Object titles across compact desktop layouts and narrow screens, even beside a lengthy relationship explanation");
  expect(title).toHaveAttribute("href", "/objects/synthetic-note");
  expect(title).toHaveAttribute("title", title.textContent);
});
