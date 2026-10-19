import { expect, test } from "vitest";
import { movedView } from "./subviews";

const saved = (sub: Record<string, string>) => (key: string) => sub[key] ?? null;

test("old nested tabs open the view they last showed", () => {
  expect(movedView("diff", saved({}))).toBe("changes");
  expect(movedView("symbols", saved({}))).toBe("changes");
  expect(movedView("reports", saved({ "lgtm:side:reports": "reviewers" }))).toBe("reviewers");
  expect(movedView("bots", saved({}))).toBe("reviewers");
  expect(movedView("files", saved({}))).toBe("changes");
  expect(movedView("design", saved({}))).toBe("qa");
  expect(movedView("repo", saved({}))).toBe("repo");
});

test("the PR and Walkthrough views open Comments", () => {
  expect(movedView("pr", saved({}))).toBe("comments");
  expect(movedView("walkthrough", saved({}))).toBe("comments");
  expect(movedView("reports", saved({}))).toBe("comments");
  expect(movedView("comments", saved({}))).toBe("comments");
});
