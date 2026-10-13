import { message } from "@tauri-apps/plugin-dialog";
import { beforeEach, expect, test, vi } from "vitest";
import { confirmAll, confirmClose, confirmWorkspace, setUnsaved } from "./unsaved";

vi.mock("@tauri-apps/plugin-dialog", () => ({ message: vi.fn() }));
const ask = vi.mocked(message);

beforeEach(() => {
  ask.mockReset();
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
