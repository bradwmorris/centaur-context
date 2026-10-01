import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { api } from "./api";
import { CorrectionHistory } from "./CorrectionHistory";

afterEach(() => vi.restoreAllMocks());
test("correction retains evidence identity and submits the read revision", async () => {
  vi.spyOn(api, "readCorrections").mockResolvedValue({ objects: [{ object: { id: "chat", kind: "chat", revision: 7, lifecycle: "active" }, corrections: [], messages: [{ id: "message", content: "Original captured wording" }] }] });
  const apply = vi.spyOn(api, "applyCorrections").mockResolvedValue({});
  const onChanged = vi.fn().mockResolvedValue(undefined);
  render(<CorrectionHistory id="chat" kind="chat" onChanged={onChanged} />);
  fireEvent.click(screen.getByRole("button", { name: "Review or correct evidence" }));
  await screen.findByLabelText("Evidence");
  fireEvent.change(screen.getByLabelText("Evidence"), { target: { value: "message:message" } });
  fireEvent.change(screen.getByLabelText("Corrected representation"), { target: { value: "Repaired transcription" } });
  fireEvent.change(screen.getByLabelText("Reason"), { target: { value: "Verified transcription correction" } });
  fireEvent.click(screen.getByRole("button", { name: "Save correction" }));
  await waitFor(() => expect(apply).toHaveBeenCalledWith([{ operation: "correct_evidence", object_id: "chat", expected_revision: 7, target_type: "message", target_id: "message", reason: "Verified transcription correction", representation: { content: "Repaired transcription" } }]));
  expect(onChanged).toHaveBeenCalledOnce();
});
