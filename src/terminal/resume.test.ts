import { describe, expect, it } from "vitest";
import { resumeCommand } from "./resume";

describe("resumeCommand", () => {
  it("resumes a session id and types nothing else into the shell", () => {
    expect(resumeCommand("0b6f1c2e-7d1a-4c55-9a1e-2f3b4c5d6e7f")).toBe(
      "claude --resume 0b6f1c2e-7d1a-4c55-9a1e-2f3b4c5d6e7f",
    );
    expect(resumeCommand("x; rm -rf ~")).toBeUndefined();
    expect(resumeCommand("")).toBeUndefined();
    expect(resumeCommand(undefined)).toBeUndefined();
  });
});
