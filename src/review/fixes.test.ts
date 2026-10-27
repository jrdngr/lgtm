import { describe, expect, it } from "vitest";
import type { CommentDto, FixDto, ThreadDto } from "../bindings";
import { fixChoices, fixComments } from "./fixes";

const fix = (id: number): FixDto => ({ id, added: 1, removed: 0, at: 0 });

function thread(id: number, fixes: FixDto[]): ThreadDto {
  return {
    id,
    path: "a.rs",
    line: 1,
    body: "",
    state: { repo: "o/n", pr: 1, stage: "review", take: null, summary: `Thread ${id}` },
    github: null,
    snippet: "",
    messages: [],
    suggestion: null,
    placed: true,
    fixes,
    createdAt: 0,
  };
}

const comment = (id: number) => ({ id }) as CommentDto;

describe("fixes", () => {
  const threads = [thread(1, [fix(7)]), thread(2, [fix(7), fix(9)]), thread(3, [])];

  it("leave only the comments on threads the shown fix is linked to", () => {
    const comments = [comment(1), comment(2), comment(3), comment(4)];
    expect(fixComments(comments, threads, 9).map((c) => c.id)).toEqual([2]);
    expect(fixComments(comments, threads, 7).map((c) => c.id)).toEqual([1, 2]);
    expect(fixComments(comments, threads)).toBe(comments);
  });

  it("are picked by thread name, with times once a thread has several", () => {
    expect(fixChoices(threads, 9).map(([id, label]) => [id, label.split(" · ")[0]])).toEqual([
      [7, "Thread 2"],
      [9, "Thread 2"],
    ]);
    expect(fixChoices([thread(1, [fix(7)])], 7)).toEqual([[7, "Thread 1"]]);
  });
});
