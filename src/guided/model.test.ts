import { describe, expect, it } from "vitest";
import type { ChunkDto, CommentDto, GuidedDto, VerdictDto } from "../bindings";
import { file, review, rows } from "../review/fixture.test-utils";
import { flatten } from "../review/model";
import {
  chunkStates,
  chunkText,
  commentedThreads,
  lineDiff,
  nextChunk,
  progressOf,
  segmentsOf,
  stepParts,
} from "./model";

// a.ts: new lines 1 ctx, 2-3 added, old 2 removed (sits at new 4), 4 ctx; then a second hunk
// whose rows are new lines 10-11.
const files = flatten(
  review([
    file("a.ts", [
      rows([" one", "+two", "+three", "-gone", " four"]),
      rows([" ten", "+eleven"], 9, 10),
    ]),
    file("b.ts", [rows(["+only"])]),
  ]),
);

const chunk = (
  id: string,
  risk: ChunkDto["risk"],
  path: string,
  start: number,
  end: number,
): ChunkDto => ({
  id,
  title: id,
  risk,
  tags: [],
  writeup: "",
  ranges: [{ path, start, end }],
  related: [],
  steps: [],
  docs: [],
  tests: null,
  promoted: false,
});

const guided = (chunks: ChunkDto[], verdicts: [string, VerdictDto][] = []): GuidedDto => ({
  chunks,
  excluded: [],
  context: [],
  verdicts,
  createdAt: null,
  stale: false,
});

const verdict = (v: VerdictDto["verdict"], text: string): VerdictDto => ({
  verdict: v,
  text,
  note: "",
  at: null,
});

describe("segmentsOf", () => {
  it("takes rows by new-side line, splitting at hunks and leaving out unknown files", () => {
    const segs = segmentsOf(files, [
      { path: "a.ts", start: 2, end: 11 },
      { path: "gone.ts", start: 1, end: 5 },
    ]);
    // Rows 1-4 of the first hunk (the removed row at new line 4 included), then the second hunk.
    expect(segs.map((s) => [s.path, s.rows])).toEqual([
      ["a.ts", [1, 2, 3, 4]],
      ["a.ts", [5, 6]],
    ]);
  });

  it("places a removed row after the last new line before it", () => {
    const segs = segmentsOf(files, [{ path: "a.ts", start: 4, end: 4 }]);
    expect(segs[0].rows.map((r) => files[0].rows[r].text)).toEqual(["gone", "four"]);
  });
});

describe("chunkStates", () => {
  const high = chunk("lock", "high", "a.ts", 2, 3);
  const low = chunk("only", "low", "b.ts", 1, 1);
  const med = chunk("tail", "medium", "a.ts", 11, 11);
  const text = (c: ChunkDto) => chunkText(files, segmentsOf(files, c.ranges));

  it("orders chunks by risk and counts their changed lines", () => {
    const states = chunkStates(files, guided([low, high, med]));
    expect(states.map((s) => [s.chunk.id, s.lines])).toEqual([
      ["lock", 2],
      ["tail", 1],
      ["only", 1],
    ]);
  });

  it("drops a verdict whose code changed since, and never counts a Change as done", () => {
    const states = chunkStates(
      files,
      guided(
        [high, low, med],
        [
          ["lock", verdict("approved", "a.ts\n+two")],
          ["only", verdict("change", text(low))],
          ["tail", verdict("skipped", text(med))],
        ],
      ),
    );
    const by = new Map(states.map((s) => [s.chunk.id, s]));
    expect([by.get("lock")?.changed, by.get("lock")?.done]).toEqual([true, false]);
    expect(by.get("only")?.done).toBe(false);
    expect(by.get("tail")?.done).toBe(true);

    const p = progressOf(states);
    expect(p).toEqual({ open: { high: 2, medium: 0, low: 1 }, approved: 0, skipped: 1, total: 4 });
  });

  it("goes to the next chunk still to review, round to the start", () => {
    const states = chunkStates(
      files,
      guided([high, low, med], [["tail", verdict("approved", text(med))]]),
    );
    expect(nextChunk(states, "lock")).toBe("only");
    expect(nextChunk(states, "only")).toBe("lock");
    const allDone = chunkStates(files, guided([high], [["lock", verdict("approved", text(high))]]));
    expect(nextChunk(allDone, "lock")).toBeUndefined();
  });
});

describe("commentedThreads", () => {
  const comment = (id: number, row: number, gh: boolean): CommentDto => ({
    id,
    path: "a.ts",
    file: 0,
    rows: { start: row, end: row },
    body: "why?",
    snippet: "",
    review: "@",
    createdAt: null,
    updatedAt: null,
    messages: [],
    suggestion: null,
    github: null,
    thread: gh ? { repo: "o/r", pr: 1, stage: "new", take: null, summary: "" } : null,
    done: false,
  });

  it("lists PR threads on code no chunk holds, and leaves out notes", () => {
    const states = chunkStates(files, guided([chunk("lock", "high", "a.ts", 2, 3)]));
    const got = commentedThreads(
      [comment(1, 1, true), comment(2, 6, true), comment(3, 6, false)],
      states,
    );
    expect(got.map((c) => c.id)).toEqual([2]);
  });
});

describe("lineDiff", () => {
  it("keeps common lines and marks the rest removed or added", () => {
    expect(lineDiff("a\nb\nc", "a\nx\nc")).toEqual([
      { kind: "same", text: "a" },
      { kind: "removed", text: "b" },
      { kind: "added", text: "x" },
      { kind: "same", text: "c" },
    ]);
  });
});

describe("stepParts", () => {
  const segs = segmentsOf(files, [
    { path: "a.ts", start: 2, end: 11 },
    { path: "b.ts", start: 1, end: 1 },
  ]);
  const related = [
    { path: "c.ts", start: 1, end: 5 },
    { path: "d.ts", start: 1, end: 5 },
  ];

  it("keeps only the code a step's lines touch", () => {
    const got = stepParts(files, segs, related, [
      { path: "a.ts", start: 11, end: 11 },
      { path: "d.ts", start: 3, end: 3 },
    ]);
    expect(got.segments.map((s) => [s.path, s.rows])).toEqual([["a.ts", [5, 6]]]);
    expect(got.related.map((r) => r.path)).toEqual(["d.ts"]);
  });

  it("shows everything for a step whose lines touch nothing", () => {
    const got = stepParts(files, segs, related, [{ path: "e.ts", start: 1, end: 1 }]);
    expect(got.segments).toBe(segs);
    expect(got.related).toBe(related);
  });
});
