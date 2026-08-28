import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ConnectionId, ObjectId } from "./ObjectIdentity";

const objectId = "11111111-1111-4111-8111-111111111111";

describe("Object identity controls", () => {
  const writeText = vi.fn().mockResolvedValue(undefined);

  beforeEach(() => {
    writeText.mockClear();
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  });

  it("labels, links, and independently copies the complete canonical UUID", async () => {
    render(<ObjectId id={objectId} compact />);
    expect(screen.getByText("Object ID")).toBeVisible();
    const link = screen.getByRole("link", { name: `Open Object ID ${objectId}` });
    expect(link).toHaveAttribute("href", `/objects/${objectId}`);
    expect(link).toHaveAttribute("title", objectId);
    expect(link).toHaveTextContent("11111111…");
    await userEvent.click(screen.getByRole("button", { name: `Copy Object ID ${objectId}` }));
    expect(writeText).toHaveBeenCalledWith(objectId);
    expect(await screen.findByText(`Copied Object ID ${objectId}`)).toBeInTheDocument();
  });

  it("routes supporting Connection IDs separately", () => {
    render(<ConnectionId id="22222222-2222-4222-8222-222222222222" />);
    expect(screen.getByRole("link", { name: "Open Connection ID 22222222-2222-4222-8222-222222222222" })).toHaveAttribute("href", "/connections/22222222-2222-4222-8222-222222222222");
  });

  it("reports clipboard failures without throwing", async () => {
    writeText.mockRejectedValueOnce(new Error("denied"));
    render(<ObjectId id={objectId} />);
    await userEvent.click(screen.getByRole("button", { name: `Copy Object ID ${objectId}` }));
    expect(await screen.findByText(`Could not copy Object ID ${objectId}`)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: `Copy Object ID ${objectId}` })).toHaveTextContent("Retry");
  });
});
