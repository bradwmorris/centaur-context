import { expect, it } from "vitest";
import { taskProject, withTaskProject } from "./taskProject";

it("reads canonical project labels without inferring from the title or prose", () => {
  expect(taskProject("Project: general\n\nCoordinate research")).toBe("general");
  expect(taskProject("Project: customer-support\r\n\r\nOutcome")).toBe("customer-support");
  expect(taskProject("Discuss Project: research")).toBeNull();
  expect(taskProject("Project: build\nProject: dev")).toBeNull();
  expect(taskProject(null)).toBeNull();
});
it("reassigns projects while preserving the execution brief and removing conflicting metadata", () => {
  const brief = "Project: research\n\nOutcome\n\nAcceptance: exact result\nProject: dev";
  const updated = withTaskProject(brief, "General");
  expect(updated).toBe("Project: general\n\nOutcome\n\nAcceptance: exact result");
  expect(taskProject(updated)).toBe("general");
  expect(() => withTaskProject(brief, "")).toThrow();
  expect(() => withTaskProject(brief, "dev\nProject: research")).toThrow();
});
