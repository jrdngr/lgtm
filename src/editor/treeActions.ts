import { message } from "@tauri-apps/plugin-dialog";
import { bottomDockOf, dockOf } from "../core/docks";
import { activate } from "../core/panels";
import type { WorkspaceName } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import { run } from "../services/pathMenu";
import { confirmAction, promptText } from "../ui/prompt";
import { baseName, type EditorParams, editorId, openFile } from "./open";
import { isUnsaved } from "./unsaved";

const isEditor = (component: string) => component === "editor";

/** Opens `path` in an editor split right of the current one, or focuses the tab it's in. */
export function openToSide(ws: WorkspaceName, path: string) {
  const api = dockOf(ws);
  if (!api) return;
  const open = api.getPanel(editorId(path));
  if (open) return activate(open);
  const active = api.activePanel;
  const editor = active && isEditor(active.view.contentComponent) ? active : undefined;
  const beside = editor ?? api.panels.find((p) => isEditor(p.view.contentComponent)) ?? active;
  const params: EditorParams = { path, nonce: Date.now() };
  api.addPanel({
    id: editorId(path),
    component: "editor",
    title: baseName(path),
    params,
    position: beside ? { referencePanel: beside, direction: "right" } : undefined,
    renderer: "always",
  });
}

/** The editors of `path` and anything under it, in either dock. */
function editorsOf(ws: WorkspaceName, path: string) {
  const prefix = editorId(path);
  return [dockOf(ws), bottomDockOf(ws)].flatMap((api) =>
    (api?.panels ?? []).filter((p) => p.id === prefix || p.id.startsWith(`${prefix}/`)),
  );
}

/** Whether `path` can move: false, after saying which, when an editor under it has unsaved
 * edits, since they could no longer be saved. */
async function noUnsaved(ws: WorkspaceName, path: string, verb: string): Promise<boolean> {
  const dirty = editorsOf(ws, path).filter((p) => isUnsaved(ws, p.id));
  if (!dirty.length) return true;
  const names = dirty.map((p) => p.id.slice("editor:".length)).join("\n");
  await message(`Save or close these first, then ${verb} again:\n\n${names}`, {
    title: "Unsaved changes",
    kind: "warning",
  });
  return false;
}

/** Closes the editors of `path` and anything under it. Returns whether `path` itself had one. */
function closeEditors(ws: WorkspaceName, path: string): boolean {
  const open = editorsOf(ws, path);
  for (const p of open) p.api.close();
  return open.some((p) => p.id === editorId(path));
}

const folderOf = (path: string) => path.slice(0, Math.max(0, path.lastIndexOf("/")));

/** Asks for a path in `dir` and makes an empty file or folder there. A file opens in the
 * editor. Calls `made` with the new path. */
export function create(ws: WorkspaceName, dir: string, folder: boolean, made: (p: string) => void) {
  run(
    (async () => {
      const path = await promptText(folder ? "New Folder" : "New File", {
        value: dir ? `${dir}/` : "",
        action: "Create",
      });
      if (!path) return;
      await unwrap(commands.createPath(ws, path, folder));
      made(path);
      if (!folder) openFile(ws, path);
    })(),
  );
}

/** Asks for a new path for `path` and moves it there. A file's open editor follows it. */
export function rename(ws: WorkspaceName, path: string, made: (p: string) => void) {
  run(
    (async () => {
      const to = await promptText(`Rename ${baseName(path)}`, { value: path, action: "Rename" });
      if (!to || to === path || !(await noUnsaved(ws, path, "rename"))) return;
      await unwrap(commands.renamePath(ws, path, to));
      made(to);
      if (closeEditors(ws, path)) openFile(ws, to);
    })(),
  );
}

/** Moves `path` into folder `dir`, keeping its name. A file's open editor follows it. */
export function moveInto(ws: WorkspaceName, path: string, dir: string, made: (p: string) => void) {
  const to = dir ? `${dir}/${baseName(path)}` : baseName(path);
  if (to === path || dir === path || dir.startsWith(`${path}/`)) return;
  run(
    (async () => {
      if (!(await noUnsaved(ws, path, "move it"))) return;
      await unwrap(commands.renamePath(ws, path, to));
      made(to);
      if (closeEditors(ws, path)) openFile(ws, to);
    })(),
  );
}

/** Moves `path` to the Trash after asking. */
export function trash(ws: WorkspaceName, path: string, done: () => void) {
  run(
    (async () => {
      if (!(await noUnsaved(ws, path, "move it to the Trash"))) return;
      if (!(await confirmAction(`Move ${path} to the Trash?`, "Move to Trash"))) return;
      await unwrap(commands.trashPath(ws, path));
      done();
      closeEditors(ws, path);
    })(),
  );
}

/** The folders that hold `path`, outermost first. */
export function foldersOf(path: string): string[] {
  const out: string[] = [];
  for (let d = folderOf(path); d; d = folderOf(d)) out.unshift(d);
  return out;
}
