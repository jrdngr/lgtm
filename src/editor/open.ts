import type { DockviewApi } from "dockview-react";
import { dockOf } from "../workbench/docks";
import { activate, inActivePane } from "../workbench/panels";
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
    return activate(open);
  }
  api.addPanel({
    id: editorId(path),
    component: "editor",
    title: baseName(path),
    params,
    position: inActivePane(api),
    // Hidden tabs keep their editor, so unsaved edits and undo history survive a tab switch.
    renderer: "always",
  });
}

/** Opens `path` in the workspace's dock. */
export function openFile(workspace: WorkspaceName, path: string, line?: number) {
  const api = dockOf(workspace);
  if (api) openFileIn(api, path, line);
}
