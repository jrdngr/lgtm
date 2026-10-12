import type { DockviewApi } from "dockview-react";
import { dockOf } from "../workbench/docks";
import type { WorkspaceName } from "../workspaces/context";

export interface EditorParams {
  path: string;
  /** 1-based line to put the cursor on. */
  line?: number;
  /** Changes on every open, so opening an open file at a line moves its cursor again. */
  nonce?: number;
}

export const editorId = (path: string) => `editor:${path}`;

export const baseName = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** Opens `path` in an editor tab, or focuses the one it's in, with the cursor at `line`. */
export function openFileIn(api: DockviewApi, path: string, line?: number) {
  const params: EditorParams = { path, line, nonce: Date.now() };
  const open = api.getPanel(editorId(path));
  if (open) {
    open.api.updateParameters(params);
    return open.api.setActive();
  }
  // Editors share a pane, opened right of the active one so what it came from stays in view.
  const editor = api.panels.find((p) => p.view.contentComponent === "editor");
  // An empty editor tab gives way to the file.
  const empty = api.panels.find(
    (p) => p.view.contentComponent === "editor" && !(p.params as Partial<EditorParams>)?.path,
  );
  const active = api.activePanel;
  const position = editor
    ? { referencePanel: editor, direction: "within" as const }
    : active && { referencePanel: active, direction: "right" as const };
  api.addPanel({
    id: editorId(path),
    component: "editor",
    title: baseName(path),
    params,
    position: position || undefined,
    // Hidden tabs keep their editor, so unsaved edits and undo history survive a tab switch.
    renderer: "always",
  });
  empty?.api.close();
}

/** Opens `path` in the workspace's dock. */
export function openFile(workspace: WorkspaceName, path: string, line?: number) {
  const api = dockOf(workspace);
  if (api) openFileIn(api, path, line);
}
