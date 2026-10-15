import { message } from "@tauri-apps/plugin-dialog";
import { beforeEach, expect, test, vi } from "vitest";
import { runningPrograms } from "../terminal/sessions";
import { confirmAll, confirmClose, confirmWorkspace, setUnsaved } from "./unsaved";

vi.mock("@tauri-apps/plugin-dialog", () => ({ message: vi.fn() }));
vi.mock("../terminal/sessions", () => ({ runningPrograms: vi.fn() }));
const ask = vi.mocked(message);
const running = vi.mocked(runningPrograms);

/** Terminals by panel id and workspace key, with what each runs. */
let terminals: [string, string, string | null][] = [];

beforeEach(() => {
  ask.mockReset();
  terminals = [];
  running.mockImplementation(async (pick) =>
    terminals.filter(([id, ws, p]) => p !== null && pick(id, ws)).map(([, , p]) => p as string),
  );
  for (const ws of [null, "feature"]) {
    for (const id of ["editor:a.ts", "editor:b.ts"]) setUnsaved(ws, id, undefined);
  }
});

const edits = (name: string, ok = true) => ({ name, save: vi.fn(async () => ok) });

test("the same file open in two workspaces is tracked separately", async () => {
  setUnsaved("feature", "editor:a.ts", edits("a.ts (feature)"));
  setUnsaved(null, "editor:a.ts", edits("a.ts"));
  setUnsaved(null, "editor:a.ts", undefined);

  expect(await confirmClose(null, "editor:a.ts")).toBe(true);
  expect(ask).not.toHaveBeenCalled();

  ask.mockResolvedValue("Cancel");
  expect(await confirmClose("feature", "editor:a.ts")).toBe(false);
  expect(await confirmWorkspace(null)).toBe(true);
  expect(ask).toHaveBeenCalledOnce();
});

test("Save All saves each file and stops at the first that fails", async () => {
  const a = edits("a.ts", false);
  const b = edits("b.ts");
  setUnsaved(null, "editor:a.ts", a);
  setUnsaved("feature", "editor:b.ts", b);
  ask.mockResolvedValue("Save All");

  expect(await confirmAll()).toBe(false);
  expect(ask.mock.calls[0][0]).toContain("2 files");
  expect(a.save).toHaveBeenCalled();
  expect(b.save).not.toHaveBeenCalled();
});

test("Don't Save lets everything go without saving", async () => {
  const a = edits("a.ts");
  setUnsaved(null, "editor:a.ts", a);
  ask.mockResolvedValue("Don't Save");

  expect(await confirmAll()).toBe(true);
  expect(a.save).not.toHaveBeenCalled();
});

test("a running program asks before its terminal, workspace or window goes", async () => {
  terminals = [
    ["term:1", "", "claude"],
    ["term:2", "", null],
    ["term:3", "feature", "npm"],
  ];
  ask.mockResolvedValue("Cancel");

  expect(await confirmClose(null, "term:2")).toBe(true);
  expect(ask).not.toHaveBeenCalled();
  expect(await confirmClose(null, "term:1")).toBe(false);
  expect(ask.mock.calls[0][0]).toBe("claude is running in a terminal.");

  ask.mockResolvedValue("Stop");
  expect(await confirmWorkspace("feature")).toBe(true);
  expect(ask.mock.calls[1][0]).toBe("npm is running in a terminal.");
  expect(await confirmAll()).toBe(true);
  expect(ask.mock.calls[2][0]).toContain("2 programs");
});

test("Cancel on unsaved edits doesn't go on to ask about programs", async () => {
  setUnsaved(null, "editor:a.ts", edits("a.ts"));
  terminals = [["term:1", "", "claude"]];
  ask.mockResolvedValue("Cancel");

  expect(await confirmAll()).toBe(false);
  expect(ask).toHaveBeenCalledOnce();
});
