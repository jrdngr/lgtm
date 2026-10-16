import { expect, test } from "vitest";
import { reportText } from "./ReportPrompt";

test("the prompt goes first, then where the review is and how to publish it", () => {
  const text = reportText("  Check the tests.\n", "trunk()..@", "/repo");
  expect(text.startsWith("Check the tests.\n\n")).toBe(true);
  expect(text).toContain("`trunk()..@` in /repo");
  expect(text).toContain("publish_findings");
});
