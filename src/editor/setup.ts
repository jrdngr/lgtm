import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { bracketMatching } from "@codemirror/language";
import { highlightSelectionMatches, search, searchKeymap } from "@codemirror/search";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
  type ViewUpdate,
} from "@codemirror/view";
import { treeSitter } from "./highlight";
import { linkClicks } from "./linkClicks";

const theme = EditorView.theme(
  {
    "&": { height: "100%", background: "var(--bg)", color: "var(--fg)" },
    ".cm-scroller": { font: "12px / 20px Menlo, monospace" },
    ".cm-gutters": { background: "var(--bg)", color: "var(--muted)", border: "none" },
    ".cm-activeLine, .cm-activeLineGutter": { background: "rgb(255 255 255 / 4%)" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": {
      background: "var(--selection) !important",
    },
    ".cm-cursor": { borderLeftColor: "var(--fg)" },
    ".cm-panels": { background: "var(--panel)", color: "var(--fg)" },
    ".cm-panels-top": { border: "none" },
  },
  { dark: true },
);

/** Holds line wrapping, so View > Word Wrap can switch it in open editors. */
export const wrapping = new Compartment();

export const wrapExtension = (on: boolean): Extension => (on ? EditorView.lineWrapping : []);

interface Hooks {
  save(): void;
  changed(u: ViewUpdate): void;
  /** Follows a link ⌘-clicked in a markdown file. */
  link(href: string): void;
  /** Opens the find bar, which then stands in for CodeMirror's own search panel. */
  find?(): void;
}

/** CodeMirror's search panel, empty: the find bar over the editor takes its place. */
const noPanel = () => ({ dom: document.createElement("div") });

const findBar = (open: () => void) => () => {
  open();
  return true;
};

/** Keeps the file's own line endings, so saving a CRLF file doesn't rewrite every line. */
export function lineEnding(text: string): Extension {
  return text.includes("\r\n") ? EditorState.lineSeparator.of("\r\n") : [];
}

export function editorExtensions(
  path: string,
  text: string,
  wrap: boolean,
  hooks: Hooks,
): Extension[] {
  const save = () => {
    hooks.save();
    return true;
  };
  return [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightActiveLine(),
    drawSelection(),
    history(),
    bracketMatching(),
    highlightSelectionMatches(),
    search(hooks.find ? { top: true, createPanel: noPanel } : { top: true }),
    keymap.of([
      { key: "Mod-s", run: save, preventDefault: true },
      ...(hooks.find ? [{ key: "Mod-f", run: findBar(hooks.find), preventDefault: true }] : []),
      indentWithTab,
      ...defaultKeymap,
      ...historyKeymap,
      ...searchKeymap,
    ]),
    lineEnding(text),
    wrapping.of(wrapExtension(wrap)),
    treeSitter(path),
    /\.(md|markdown|mdx)$/i.test(path) ? linkClicks(hooks.link) : [],
    theme,
    EditorView.updateListener.of(hooks.changed),
  ];
}
