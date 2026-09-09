import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { DescriptionSnippet, normalizeDescription, truncateDescription } from "./DescriptionSnippet";

describe("DescriptionSnippet", () => {
  it("normalizes multiline Unicode text and identifies itself as a preview", () => {
    render(<DescriptionSnippet description={"  Montréal\nlaunch   approved  "} />);
    const snippet = screen.getByText("Montréal launch approved");
    expect(snippet).toHaveAccessibleName(/Description preview: Montréal launch approved/);
    expect(normalizeDescription("one\n\ttwo")).toBe("one two");
  });

  it("truncates long and unbroken descriptions without changing the input", () => {
    const original = `${"word ".repeat(60)}終`;
    const preview = truncateDescription(original);
    expect(preview).toMatch(/…$/);
    expect(Array.from(preview).length).toBeLessThanOrEqual(221);
    expect(original).toMatch(/終$/);

    const unbroken = "界".repeat(300);
    expect(Array.from(truncateDescription(unbroken)).length).toBe(221);
  });

  it("shows an explicit fallback for a missing legacy value", () => {
    render(<DescriptionSnippet description="  " />);
    expect(screen.getByText("Description unavailable")).toHaveAttribute("data-empty", "true");
  });
});
