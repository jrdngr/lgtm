import { message } from "@tauri-apps/plugin-dialog";
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

/** Whether panel `id` may close: true when it has no unsaved edits, or the user saved or
 * discarded them. */
export function confirmClose(workspace: WorkspaceName, id: string): Promise<boolean> {
  const edits = unsaved.get(keyOf(workspace, id));
  return confirmDiscard(edits ? [edits] : []);
}

/** Whether the workspace's editors may go, as `confirmClose` asks for one. */
export function confirmWorkspace(workspace: WorkspaceName): Promise<boolean> {
  const key = workspaceKey(workspace);
  return confirmDiscard([...unsaved.values()].filter((u) => u.workspace === key));
}

/** Whether every editor in the window may go, as `confirmClose` asks for one. */
export function confirmAll(): Promise<boolean> {
  return confirmDiscard([...unsaved.values()]);
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
