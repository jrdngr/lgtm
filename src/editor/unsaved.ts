import { terminalTitle } from "../terminal/registry";
import { type Running, runningPrograms } from "../terminal/sessions";
import { tabAgent } from "../workspaces/activity";
import { type WorkspaceName, workspaceKey } from "../workspaces/context";
import { askClose } from "./CloseDialog";

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
  return confirm(edits ? [edits] : [], await runningPrograms((panel) => panel === id));
}

/** Whether the workspace's editors and terminals may go, as `confirmClose` asks for one. */
export async function confirmWorkspace(workspace: WorkspaceName): Promise<boolean> {
  const key = workspaceKey(workspace);
  return confirm(
    [...unsaved.values()].filter((u) => u.workspace === key),
    await runningPrograms((_, ws) => ws === key),
  );
}

/** Whether every editor and terminal in the window may go, as `confirmClose` asks for one. */
export async function confirmAll(): Promise<boolean> {
  return confirm([...unsaved.values()], await runningPrograms(() => true));
}

/** Asks once about `files` and `running`. Saving saves every file and stops at the first that
 * fails. Each terminal's agent is listed like the workspace's agent list, and any other
 * program by name. */
async function confirm(files: Unsaved[], running: Running[]): Promise<boolean> {
  if (files.length === 0 && running.length === 0) return true;
  const programs = running.map(({ panel, program }) => {
    const agent = tabAgent(panel);
    return { key: panel, name: agent ? (terminalTitle(panel) ?? agent.label) : program, agent };
  });
  return askClose(
    files.map((f) => f.name),
    programs,
    async () => {
      for (const f of files) {
        if (!(await f.save())) return false;
      }
      return true;
    },
  );
}
