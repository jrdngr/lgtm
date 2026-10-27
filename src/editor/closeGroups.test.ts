import { expect, test } from "vitest";
import { grouped } from "./closeGroups";

test("rows keep their order under the first place each repo and workspace shows up", () => {
  const at = (repo: string, workspace: string, key: string) => ({
    key,
    where: { repo, workspace },
  });
  const groups = grouped([
    at("lgtm", "default", "a"),
    at("agent-stuff", "default", "b"),
    at("lgtm", "quit", "c"),
    at("lgtm", "default", "d"),
  ]);
  expect(
    groups.map((r) => [r.repo, r.workspaces.map((w) => [w.workspace, w.rows.map((x) => x.key)])]),
  ).toEqual([
    [
      "lgtm",
      [
        ["default", ["a", "d"]],
        ["quit", ["c"]],
      ],
    ],
    ["agent-stuff", [["default", ["b"]]]],
  ]);
});
