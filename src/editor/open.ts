import type { DockviewApi } from "dockview-react";
import { dockOf } from "../core/docks";
import { activate, inActivePane } from "../core/panels";
import type { WorkspaceName } from "../core/workspace";

export interface EditorParams {
  path: string;
  /** 1-based line to put the cursor on. */
  line?: number;
  /** The cursor's UTF-16 column on `line`. */
  col?: number;
  /** A library file a definition led to, outside the workspace. */
  readOnly?: boolean;
  /** A link's `#anchor` to show: a heading, an element id, or a line (`L12`). */
  anchor?: string;
  /** Changes on every open, so opening an open file at a line moves its cursor again. */
  nonce?: number;
  // Kept with the layout, so the tab comes back as it was.
  view?: "editor" | "rendered";
  /** The cursor's offset and the editor's scroll. */
  cursor?: number;
  scroll?: number;
  /** Rendered markdown's scroll. */
  renderedScroll?: number;
  /** An image's or HTML page's zoom; images shrink to fit without one. */
  zoom?: "fit" | number;
  /** The HTML page showing, as its URL after the root id, and its scroll. */
  page?: string;
  pageScroll?: [number, number];
}

export const editorId = (path: string) => `editor:${path}`;

export const baseName = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** Opens `path` in an editor tab, or focuses the one it's in, with the cursor at `line` or
 * showing `anchor`. */
export function openFileIn(
  api: DockviewApi,
  path: string,
  line?: number,
  anchor?: string,
  at?: Pick<EditorParams, "col" | "readOnly">,
) {
  const params: EditorParams = { path, line, anchor, nonce: Date.now(), ...at };
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
export function openFile(
  workspace: WorkspaceName,
  path: string,
  line?: number,
  anchor?: string,
  at?: Pick<EditorParams, "col" | "readOnly">,
) {
  const api = dockOf(workspace);
  if (api) openFileIn(api, path, line, anchor, at);
}
