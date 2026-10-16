import { selectAll } from "@codemirror/commands";
import type { EditorView } from "@codemirror/view";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { commands } from "../ipc";
import { currentReview } from "../review/current";
import { type Sender, sendItems } from "../terminal/sendItems";
import { type CtxEntry, clipboardText, copy } from "../ui/contextMenu";
import { wordWrap } from "../workbench/wordWrap";
import type { WorkspaceName } from "../workspaces/context";
import { revealInFiles } from "./reveal";

interface Deps {
  view: EditorView;
  workspace: WorkspaceName;
  path: string;
  dirty: boolean;
  save(): void;
  /** Pastes text into one of the workspace's terminals. */
  send: Sender;
  /** Opens the agent box under the selection, to ask about it or to change it. */
  askAgent(change: boolean): void;
  /** Set for markdown, which can show rendered. */
  toggleRendered?: () => void;
}

const selected = (v: EditorView) =>
  v.state.selection.ranges.map((r) => v.state.sliceDoc(r.from, r.to)).join("\n");

/** Puts the cursor where a right-click landed, unless it landed in the selection. */
export function placeAt(v: EditorView, e: MouseEvent | React.MouseEvent) {
  const pos = v.posAtCoords({ x: e.clientX, y: e.clientY });
  if (pos === null) return;
  const main = v.state.selection.main;
  if (pos < main.from || pos > main.to) v.dispatch({ selection: { anchor: pos } });
}

function paste(v: EditorView) {
  clipboardText().then(
    (text) => v.dispatch(v.state.replaceSelection(text), { userEvent: "input.paste" }),
    (e) => console.error("paste", e),
  );
}

/** `path:line`, or `path:start-end` over a selection of several lines. */
function pathAndLine(v: EditorView, path: string): string {
  const { from, to } = v.state.selection.main;
  const [a, b] = [v.state.doc.lineAt(from).number, v.state.doc.lineAt(to).number];
  return `${path}:${a === b ? a : `${a}-${b}`}`;
}

/** The right-click menu of an editor tab's text. */
export function editorMenu(d: Deps): CtxEntry[] {
  const { view: v, path } = d;
  const empty = v.state.selection.ranges.every((r) => r.empty);
  const review = currentReview(d.workspace);
  const inReview = review?.files.some((f) => f.dto.path === path || f.dto.oldPath === path);
  const line = v.state.doc.lineAt(v.state.selection.main.head).number;
  const after = (fn: () => void) => () => {
    fn();
    v.focus();
  };
  const wrap = wordWrap.get();
  return [
    {
      label: "Cut",
      keys: "⌘X",
      disabled: empty,
      onSelect: after(() => {
        // The text goes only once it's on the clipboard.
        const range = v.state.selection.main;
        writeText(selected(v)).then(
          () => v.dispatch({ changes: range, userEvent: "delete.cut" }),
          (e) => console.error("cut", e),
        );
      }),
    },
    { label: "Copy", keys: "⌘C", disabled: empty, onSelect: after(() => copy(selected(v))) },
    { label: "Paste", keys: "⌘V", onSelect: after(() => paste(v)) },
    { label: "Select All", keys: "⌘A", onSelect: after(() => selectAll(v)) },
    "-",
    ...sendItems(d.workspace, "Send", pathAndLine(v, path), d.send),
    { label: "Ask Agent", onSelect: () => d.askAgent(false) },
    { label: "Change with Agent", onSelect: () => d.askAgent(true) },
    "-",
    { label: "Copy Path and Line", onSelect: () => copy(pathAndLine(v, path)) },
    { label: "Reveal in Files", onSelect: () => revealInFiles(d.workspace, path) },
    {
      label: "Show Line in Review",
      disabled: !inReview,
      onSelect: () => review?.show(path, line),
    },
    "-",
    { label: "Save", keys: "⌘S", disabled: !d.dirty, onSelect: d.save },
    {
      label: "Word Wrap",
      keys: "⌥Z",
      checked: wrap,
      onSelect: () => {
        wordWrap.set(!wrap);
        commands.setWordWrap(!wrap);
      },
    },
    d.toggleRendered && { label: "Show Rendered", keys: "⇧⌘V", onSelect: d.toggleRendered },
  ];
}
