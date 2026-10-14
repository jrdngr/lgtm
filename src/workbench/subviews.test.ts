import { expect, test } from "vitest";
import { movedView } from "./subviews";

test("old nested tabs open the view they last showed", () => {
  const saved = (sub: Record<string, string>) => (key: string) => sub[key] ?? null;
  expect(movedView("diff", saved({}))).toBe("changes");
  expect(movedView("diff", saved({ "lgtm:side:diff": "symbols" }))).toBe("symbols");
  expect(movedView("reports", saved({}))).toBe("walkthrough");
  expect(movedView("reports", saved({ "lgtm:side:reports": "reviewers" }))).toBe("reviewers");
  expect(movedView("bots", saved({}))).toBe("reviewers");
  expect(movedView("files", saved({}))).toBe("changes");
  expect(movedView("design", saved({}))).toBe("qa");
  expect(movedView("repo", saved({}))).toBe("repo");
});
