import { describe, expect, it } from "vitest";
import type { AgentCommentDto } from "../bindings";
import { agentDraftFor, agentSlots, NEW_AGENT_COMMENT, openCounts } from "./agentComments";
import { file, layout, review, rows } from "./fixture.test-utils";
import { buildLines, flatten } from "./model";

const files = flatten(review([file("a.rs", [rows([" a", "-b", "+B", " c", "+d"])])]));
const lines = buildLines(files, layout());

const comment = (number: number, end: number | null, done = false) =>
  ({
    number,
    path: "a.rs",
    file: 0,
    rows: end === null ? null : { start: end, end },
    body: "",
    done,
    createdAt: 0,
  }) as AgentCommentDto;

describe("comments for the agent", () => {
  it("can go on unchanged rows, which drafts for GitHub can't", () => {
    // Line 2 holds row 0, " a".
    expect(agentDraftFor(files, lines, 2, 2)).toMatchObject({ file: 0, rows: [0] });
    expect(agentDraftFor(files, lines, 0, 1)).toBeUndefined();
  });

  it("show resolved ones only when asked, ones whose code is gone under the header", () => {
    const cs = [comment(1, 2), comment(2, 2, true), comment(3, null)];
    const draft = { file: 0, rows: [1, 2], snippet: "" };
    expect(agentSlots(cs, draft, false)).toEqual(
      new Map([
        ["0:2", [-1, NEW_AGENT_COMMENT]],
        ["0:file", [-3]],
      ]),
    );
    expect(agentSlots(cs, undefined, true).get("0:2")).toEqual([-1, -2]);
    expect(openCounts(cs)).toEqual(new Map([[0, 2]]));
  });
});
