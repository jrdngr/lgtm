import { beforeEach, expect, test, vi } from "vitest";
import { runningPrograms } from "../terminal/sessions";
import { report } from "../workspaces/activity";
import { askClose } from "./CloseDialog";
import { confirmAll, confirmClose, confirmWorkspace, setUnsaved } from "./unsaved";

vi.mock("./CloseDialog", () => ({ askClose: vi.fn() }));
vi.mock("../terminal/sessions", () => ({ runningPrograms: vi.fn() }));
vi.mock("../terminal/registry", () => ({
  terminalTitle: (id: string) => (id === "term:1" ? "Fix login" : undefined),
}));
const ask = vi.mocked(askClose);
const running = vi.mocked(runningPrograms);

/** Terminals by panel id and workspace key, with what each runs. */
let terminals: [string, string, string | null][] = [];

beforeEach(() => {
  ask.mockReset();
  terminals = [];
  running.mockImplementation(async (pick) =>
    terminals
      .filter(([id, ws, p]) => p !== null && pick(id, ws))
      .map(([panel, , p]) => ({ panel, program: p as string })),
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

  ask.mockResolvedValue("cancel");
  expect(await confirmClose("feature", "editor:a.ts")).toBe(false);
  expect(await confirmWorkspace(null)).toBe(true);
  expect(ask).toHaveBeenCalledOnce();
});

test("Save All saves each file and stops at the first that fails", async () => {
  const a = edits("a.ts", false);
  const b = edits("b.ts");
  setUnsaved(null, "editor:a.ts", a);
  setUnsaved("feature", "editor:b.ts", b);
  ask.mockResolvedValue("save");

  expect(await confirmAll()).toBe(false);
  expect(ask.mock.calls[0][0]).toEqual(["a.ts", "b.ts"]);
  expect(a.save).toHaveBeenCalled();
  expect(b.save).not.toHaveBeenCalled();
});

test("Don't Save lets everything go without saving", async () => {
  const a = edits("a.ts");
  setUnsaved(null, "editor:a.ts", a);
  ask.mockResolvedValue("discard");

  expect(await confirmAll()).toBe(true);
  expect(a.save).not.toHaveBeenCalled();
});

test("a running program asks before its terminal, workspace or window goes", async () => {
  terminals = [
    ["term:1", "", "claude"],
    ["term:2", "", null],
    ["term:3", "feature", "npm"],
  ];
  report(null, "term-agent", "running", "claude", { stays: true, tab: "term:1" });
  ask.mockResolvedValue("cancel");

  expect(await confirmClose(null, "term:2")).toBe(true);
  expect(ask).not.toHaveBeenCalled();
  expect(await confirmClose(null, "term:1")).toBe(false);
  const [files, [agent]] = ask.mock.calls[0];
  expect(files).toEqual([]);
  expect(agent).toMatchObject({ name: "Fix login", agent: { state: "working" } });

  ask.mockResolvedValue("stop");
  expect(await confirmWorkspace("feature")).toBe(true);
  expect(ask.mock.calls[1][1]).toEqual([{ key: "term:3", name: "npm", agent: undefined }]);
  expect(await confirmAll()).toBe(true);
  expect(ask.mock.calls[2][1]).toHaveLength(2);
});

test("unsaved edits and running programs are asked about together", async () => {
  setUnsaved(null, "editor:a.ts", edits("a.ts"));
  terminals = [["term:3", "", "npm"]];
  ask.mockResolvedValue("cancel");

  expect(await confirmAll()).toBe(false);
  expect(ask).toHaveBeenCalledOnce();
  expect(ask.mock.calls[0]).toEqual([["a.ts"], [{ key: "term:3", name: "npm", agent: undefined }]]);
});
