import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "./api";
import { InlineEditor } from "./InlineEditor";

afterEach(() => vi.useRealTimers());

describe("InlineEditor", () => {
  it("autosaves after 800ms and suppresses unchanged writes", async () => {
    vi.useFakeTimers();
    const save = vi.fn().mockResolvedValue(undefined);
    render(<InlineEditor label="Title" value="Original" onSave={save} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit Title" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: "Changed" } });
    await act(async () => vi.advanceTimersByTime(799));
    expect(save).not.toHaveBeenCalled();
    await act(async () => vi.advanceTimersByTime(1));
    expect(save).toHaveBeenCalledWith("Changed");
    fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: "Changed" } });
    await act(async () => vi.advanceTimersByTime(800));
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("supports immediate keyboard save and Escape restoration", async () => {
    const save = vi.fn().mockResolvedValue("Canonical");
    render(<InlineEditor label="Title" value="Original" onSave={save} />);
    await userEvent.click(screen.getByRole("button", { name: "Edit Title" }));
    const input = screen.getByRole("textbox", { name: "Title" });
    await userEvent.clear(input); await userEvent.type(input, "Draft{Enter}");
    expect(save).toHaveBeenCalledWith("Draft");
    await userEvent.clear(input); await userEvent.type(input, "Discard{Escape}");
    expect(screen.getByRole("button", { name: "Edit Title" })).toHaveTextContent("Canonical");
  });

  it("serializes writes, coalesces to the latest draft, and preserves conflicts", async () => {
    let resolveFirst!: (value?: string) => void;
    const first = new Promise<string | void>((resolve) => { resolveFirst = resolve; });
    const save = vi.fn().mockReturnValueOnce(first).mockResolvedValueOnce(undefined).mockRejectedValueOnce(new ApiError("conflict", 409));
    render(<InlineEditor label="Body" value="One" multiline onSave={save} />);
    await userEvent.click(screen.getByRole("button", { name: "Edit Body" }));
    const input = screen.getByRole("textbox", { name: "Body" });
    fireEvent.change(input, { target: { value: "Two" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Body" }));
    fireEvent.change(input, { target: { value: "Three" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Body" }));
    expect(save).toHaveBeenCalledTimes(1);
    await act(async () => resolveFirst("Two"));
    expect(save).toHaveBeenLastCalledWith("Three");
    fireEvent.change(input, { target: { value: "Conflict draft" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Body" }));
    expect(await screen.findByText(/Server data changed/)).toBeVisible();
    expect(input).toHaveValue("Conflict draft");
  });

  it("saves on blur, keeps drafts across refreshes, and retries network errors", async () => {
    const save = vi.fn().mockRejectedValueOnce(new Error("Offline")).mockResolvedValueOnce(undefined);
    const view = render(<InlineEditor label="Body" value="Server one" multiline onSave={save} />);
    await userEvent.click(screen.getByRole("button", { name: "Edit Body" }));
    const input = screen.getByRole("textbox", { name: "Body" });
    fireEvent.change(input, { target: { value: "Local draft" } });
    view.rerender(<InlineEditor label="Body" value="Server two" multiline onSave={save} />);
    expect(input).toHaveValue("Local draft");
    fireEvent.blur(input);
    expect(await screen.findByText("Offline")).toBeVisible();
    expect(input).toHaveValue("Local draft");
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(save).toHaveBeenLastCalledWith("Local draft");
  });
});
