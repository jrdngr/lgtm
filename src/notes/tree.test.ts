import { describe, expect, it } from "vitest";
import type { NoteDto, NotesDto } from "../bindings";
import { stampName } from "./store";
import { levelTree, parentOf } from "./tree";

const note = (id: string, level: NoteDto["level"], folder: string): NoteDto => ({
  id,
  level,
  folder,
  name: id,
  body: "",
  createdAt: 0,
  updatedAt: 0,
});

describe("levelTree", () => {
  it("nests one level's folders under their parents and files its notes in them", () => {
    const data: NotesDto = {
      notes: [note("a", "repo", "Plans/Auth"), note("b", "repo", ""), note("c", "global", "")],
      folders: [
        { level: "repo", path: "Plans/Auth" },
        { level: "repo", path: "Plans" },
        { level: "repo", path: "Archive" },
        { level: "global", path: "Elsewhere" },
      ],
    };
    const root = levelTree(data, "repo");
    expect(root.notes.map((n) => n.id)).toEqual(["b"]);
    expect(root.folders.map((f) => f.name)).toEqual(["Archive", "Plans"]);
    expect(root.folders[1].folders[0].notes.map((n) => n.id)).toEqual(["a"]);
  });
});

it("parentOf drops the last segment", () => {
  expect(parentOf("a/b/c")).toBe("a/b");
  expect(parentOf("a")).toBe("");
});

it("stampName names a quick note by its local minute", () => {
  expect(stampName(new Date(2026, 9, 1, 7, 5))).toBe("2026-10-01 07:05");
});
