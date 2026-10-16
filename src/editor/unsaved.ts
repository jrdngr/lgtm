import { message } from "@tauri-apps/plugin-dialog";
import { runningPrograms } from "../terminal/sessions";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";

interface Unsaved {
  workspace: string;
  /** What the dialog calls the file. */
  name: string;
  /** Resolves to whether the save worked. */
  save(): Promise<boolean>;
}

/** Editor panels with unsaved edits, by workspace and panel id. */
const unsaved = new Map<string, Unsaved>();

const keyOf = (workspace: WorkspaceName, id: string) => `${workspaceKey(workspace)}\n${id}`;

export function setUnsaved(
  workspace: WorkspaceName,
  id: string,
  edits: { name: string; save(): Promise<boolean> } | undefined,
) {
  const key = keyOf(workspace, id);
  if (edits) unsaved.set(key, { workspace: workspaceKey(workspace), ...edits });
  else unsaved.delete(key);
}

export const isUnsaved = (workspace: WorkspaceName, id: string) =>
  unsaved.has(keyOf(workspace, id));

/** Saves panel `id`'s unsaved edits, if it has any. */
export const saveEditor = (workspace: WorkspaceName, id: string) =>
  unsaved.get(keyOf(workspace, id))?.save();

/** Whether panel `id` may close: true when it has no unsaved edits and runs no program, or
 * the user saved or discarded the edits and agreed to stop the program. */
export async function confirmClose(workspace: WorkspaceName, id: string): Promise<boolean> {
  const edits = unsaved.get(keyOf(workspace, id));
  return (
    (await confirmDiscard(edits ? [edits] : [])) &&
    confirmStop(await runningPrograms((panel) => panel === id))
  );
}

/** Whether the workspace's editors and terminals may go, as `confirmClose` asks for one. */
export async function confirmWorkspace(workspace: WorkspaceName): Promise<boolean> {
  const key = workspaceKey(workspace);
  return (
    (await confirmDiscard([...unsaved.values()].filter((u) => u.workspace === key))) &&
    confirmStop(await runningPrograms((_, ws) => ws === key))
  );
}

/** Whether every editor and terminal in the window may go, as `confirmClose` asks for one. */
export async function confirmAll(): Promise<boolean> {
  return (
    (await confirmDiscard([...unsaved.values()])) &&
    confirmStop(await runningPrograms(() => true, true))
  );
}

async function confirmStop(programs: string[]): Promise<boolean> {
  if (programs.length === 0) return true;
  const text =
    programs.length === 1
      ? `${programs[0]} is running in a terminal.`
      : `${programs.length} programs are running in terminals:\n\n${programs.join("\n")}`;
  const choice = await message(text, {
    title: "Stop running programs?",
    kind: "warning",
    buttons: { ok: "Stop", cancel: "Cancel" },
  });
  return choice === "Stop";
}

async function confirmDiscard(files: Unsaved[]): Promise<boolean> {
  if (files.length === 0) return true;
  const text =
    files.length === 1
      ? `${files[0].name} has unsaved changes.`
      : `${files.length} files have unsaved changes:\n\n${files.map((f) => f.name).join("\n")}`;
  const choice = await message(text, {
    title: "Save changes?",
    kind: "warning",
    buttons: { yes: files.length === 1 ? "Save" : "Save All", no: "Don't Save", cancel: "Cancel" },
  });
  if (choice === "Don't Save") return true;
  if (choice !== "Save" && choice !== "Save All") return false;
  for (const f of files) {
    if (!(await f.save())) return false;
  }
  return true;
}
