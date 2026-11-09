/** Modal editing in CodeMirror: the TUI editor's keys (see `keys.ts`) on the editor's text,
 * with a bar at the bottom for the mode, key menus and the `:` and `/` lines. CodeMirror
 * types the text in insert mode; the keys handle everything else. */

import { codeFolding, foldEffect, foldedRanges, unfoldEffect } from "@codemirror/language";
import {
  EditorSelection,
  type Extension,
  Prec,
  StateEffect,
  StateField,
  type Text,
} from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  type Panel,
  showPanel,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from "@codemirror/view";
import { runChord } from "../../core/chords";
import { logger } from "../../core/log";
import { remembered } from "../../core/remembered";
import { commands } from "../../ipc";
import { clipboardText, copy } from "../../ui/contextMenu";
import { Buffer, Sel } from "./buffer";
import { Folds, indentSpans, ranges } from "./folds";
import { after, type Key } from "./keymap";
import { type Action, Keys, type SyntaxOp } from "./keys";

const log = logger("modal");

/** Whether editors use the modal keys, app-wide. */
export const modalEditing = remembered("lgtm:modalEditing", true, [true, false]);

/** What the modal keys ask of the editor's tab. */
export interface ModalHooks {
  save(): void;
  /** Closes the tab. */
  close?(): void;
  /** Opens a file of the workspace. */
  open?(path: string): void;
  /** Shows a line, from 1, in the review. */
  review?(line: number): void;
  split?(right: boolean): void;
  /** Keys that would change the text do nothing. */
  readOnly?: boolean;
}

/** One editor's keys, its text as they see it, and the bar's state. */
class Modal {
  keys: Keys;
  buf: Buffer;
  folds = new Folds();
  foldsAt = -1;
  /** The document `buf` was last synced with. */
  doc: Text | undefined;
  notice: string | undefined;
  /** The `:` or `/` line, while it's open. */
  line: { search: boolean } | undefined;

  constructor(
    readonly path: string,
    readonly hooks: ModalHooks,
  ) {
    this.buf = new Buffer("", path);
    this.keys = new Keys(hooks.readOnly);
  }
}

const modals = new WeakMap<EditorView, Modal>();
/** Redraws the bar and the cursor after the keys change state but not the text. */
const tick = StateEffect.define<null>();
const onEffect = StateEffect.define<boolean>();

const onField = StateField.define<boolean>({
  create: () => modalEditing.get(),
  update: (on, tr) => tr.effects.reduce((v, e) => (e.is(onEffect) ? e.value : v), on),
});

/** The keys' selection as CodeMirror's: a range over the chars from anchor to head. */
function toCm(m: Modal, len: number) {
  const { anchor, head } = m.buf.sel;
  if (m.keys.mode === "insert" || len === 0) return EditorSelection.cursor(Math.min(head, len));
  return head >= anchor
    ? EditorSelection.range(anchor, Math.min(head + 1, len))
    : EditorSelection.range(Math.min(anchor + 1, len), head);
}

/** CodeMirror's selection as the keys see it. */
function fromCm(view: EditorView, m: Modal): Sel {
  const { anchor, head } = view.state.selection.main;
  if (m.keys.mode === "insert" || anchor === head) return Sel.point(head);
  return head > anchor ? new Sel(anchor, head - 1) : new Sel(anchor - 1, head);
}

/** Brings the keys' text and selection up to the editor's. */
function sync(view: EditorView, m: Modal) {
  if (m.doc !== view.state.doc) {
    m.buf.setText(view.state.doc.toString());
    m.doc = view.state.doc;
  }
  m.buf.sel = fromCm(view, m);
  if (m.keys.mode !== "insert") m.buf.clamp();
}

/** Puts the keys' text and selection in the editor. */
function apply(view: EditorView, m: Modal) {
  const before = view.state.doc.toString();
  const text = m.buf.text;
  let changes: { from: number; to: number; insert: string } | undefined;
  if (text !== before) {
    let start = 0;
    while (start < before.length && start < text.length && before[start] === text[start]) start++;
    let end = 0;
    while (
      end < before.length - start &&
      end < text.length - start &&
      before[before.length - 1 - end] === text[text.length - 1 - end]
    )
      end++;
    changes = {
      from: start,
      to: before.length - end,
      insert: text.slice(start, text.length - end),
    };
  }
  view.dispatch({
    changes,
    selection: toCm(m, text.length),
    scrollIntoView: true,
    userEvent: changes ? "input.modal" : "select.modal",
    effects: tick.of(null),
  });
  m.doc = view.state.doc;
}

/** The lines closed folds hide, from 0. */
function hidden(view: EditorView): [number, number][] {
  const out: [number, number][] = [];
  const doc = view.state.doc;
  foldedRanges(view.state).between(0, doc.length, (from, to) => {
    out.push([doc.lineAt(from).number - 1, doc.lineAt(to).number - 1]);
  });
  return out;
}

/** The key the keys see for a key event: Alt keys by their letter, as macOS types a symbol
 * for them. */
function keyOf(e: KeyboardEvent): Key {
  let key = e.key;
  if (e.altKey && /^Key[A-Z]$/.test(e.code)) {
    key = e.shiftKey ? e.code[3] : e.code[3].toLowerCase();
  }
  return { key, ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey };
}

const encoder = new TextEncoder();
const bytesBefore = (text: string, at: number) => encoder.encode(text.slice(0, at)).length;
const charsBefore = (text: string, bytes: number) =>
  new TextDecoder().decode(encoder.encode(text).slice(0, bytes)).length;

async function syntax(view: EditorView, m: Modal, op: SyntaxOp) {
  const text = m.buf.text;
  const { from, to } = { from: m.buf.sel.from(), to: m.buf.sel.to() + 1 };
  const found = await commands.syntaxNode(
    m.path,
    text,
    bytesBefore(text, from),
    bytesBefore(text, Math.min(to, text.length)),
    op,
  );
  if (m.buf.text !== text) return;
  const range: [number, number] | undefined = found
    ? [charsBefore(text, found[0]), Math.max(charsBefore(text, found[1]) - 1, 0)]
    : undefined;
  sync(view, m);
  act(view, m, m.keys.syntaxFound(m.buf, op, range));
}

/** Finds the fold ranges again when the text changed since. */
async function freshFolds(m: Modal) {
  if (m.foldsAt === m.buf.version) return;
  const text = m.buf.text;
  const spans = await commands.foldSpans(m.path, text).catch(() => null);
  m.folds.update(ranges(spans ? new Map(spans) : indentSpans(m.buf)));
  m.foldsAt = m.buf.version;
}

/** Shows the closed folds the keys keep, and only them. */
function showFolds(view: EditorView, m: Modal) {
  const doc = view.state.doc;
  const effects: StateEffect<unknown>[] = [];
  foldedRanges(view.state).between(0, doc.length, (from, to) => {
    effects.push(unfoldEffect.of({ from, to }));
  });
  for (const [start, end] of m.folds.hidden()) {
    if (end + 1 > doc.lines) continue;
    effects.push(foldEffect.of({ from: doc.line(start + 1).to, to: doc.line(end + 1).to }));
  }
  view.dispatch({ effects: [...effects, tick.of(null)] });
}

async function fold(view: EditorView, m: Modal, f: Parameters<Folds["apply"]>[0]) {
  await freshFolds(m);
  m.folds.apply(f, m.buf.lineOf(m.buf.sel.head));
  showFolds(view, m);
}

/** Does what a key's action asks of the editor. */
function act(view: EditorView, m: Modal, action: Action) {
  apply(view, m);
  const line = m.buf.lineOf(m.buf.sel.head) + 1;
  switch (action.a) {
    case "copy":
      copy(action.text);
      break;
    case "paste":
      clipboardText().then((text) => {
        sync(view, m);
        m.keys.pasteText(m.buf, text);
        apply(view, m);
      });
      break;
    case "notice":
      m.notice = action.text;
      view.dispatch({ effects: tick.of(null) });
      break;
    case "fold":
      fold(view, m, action.fold).catch((e) => log.error("fold", e));
      break;
    case "pick":
      runChord(action.pick === "grep" ? "Mod+Shift+F" : "Mod+P");
      break;
    case "review":
      m.hooks.review?.(line);
      break;
    case "sidebar":
      runChord("Mod+Shift+R");
      break;
    case "toggleSidebar":
      runChord("Mod+Alt+B");
      break;
    case "split":
      m.hooks.split?.(action.right);
      break;
    case "syntax":
      syntax(view, m, action.op).catch((e) => log.error("syntax", e));
      break;
    case "command":
    case "search":
      m.line = { search: action.a === "search" };
      view.dispatch({ effects: tick.of(null) });
      break;
    case "none":
      break;
  }
}

/** Runs what was typed on the `:` line. */
function command(view: EditorView, m: Modal, text: string) {
  const [name, ...rest] = text.trim().split(/\s+/);
  const arg = rest.join(" ");
  if (/^\d+$/.test(name)) {
    const line = Math.min(Math.max(Number(name), 1), view.state.doc.lines);
    const at = view.state.doc.line(line).from;
    m.buf.sel = Sel.point(at);
    return apply(view, m);
  }
  if (name === "w" || name === "wq") m.hooks.save();
  if (name === "q" || name === "wq") m.hooks.close?.();
  else if (name === "e" && arg) m.hooks.open?.(arg);
  else if (name !== "w") m.notice = `not a command: ${name}`;
  view.dispatch({ effects: tick.of(null) });
}

function onKey(e: KeyboardEvent, view: EditorView): boolean {
  const m = modals.get(view);
  if (!m || !view.state.field(onField) || e.isComposing || e.metaKey) return false;
  if (["Shift", "Alt", "Control", "Meta", "CapsLock"].includes(e.key)) return false;
  const insert = m.keys.mode === "insert";
  if (insert && e.key !== "Escape" && !(e.ctrlKey && e.key === "w")) return false;
  sync(view, m);
  m.notice = undefined;
  const height = Math.max(Math.floor(view.scrollDOM.clientHeight / view.defaultLineHeight), 1);
  const action = m.keys.press(m.buf, keyOf(e), { height, hidden: hidden(view) });
  e.preventDefault();
  act(view, m, action);
  revealCursor(view, m);
  return true;
}

/** Opens the folds that hide the cursor's line. */
function revealCursor(view: EditorView, m: Modal) {
  const line = m.buf.lineOf(m.buf.sel.head);
  const hides = hidden(view).some(([s, e]) => s < line && line <= e);
  if (!hides) return;
  m.folds.reveal(line);
  showFolds(view, m);
}

class EolWidget extends WidgetType {
  toDOM() {
    const span = document.createElement("span");
    span.className = "cm-modal-eol";
    return span;
  }
  eq() {
    return true;
  }
}

/** The block cursor on the head's char in normal and select mode. */
function cursorDeco(view: EditorView): DecorationSet {
  const m = modals.get(view);
  if (!m || !view.state.field(onField) || m.keys.mode === "insert") return Decoration.none;
  const { anchor, head } = view.state.selection.main;
  const at = head > anchor ? head - 1 : head;
  const doc = view.state.doc;
  if (at >= doc.length || doc.sliceString(at, at + 1) === "\n") {
    return Decoration.set([Decoration.widget({ widget: new EolWidget(), side: 1 }).range(at)]);
  }
  return Decoration.set([Decoration.mark({ class: "cm-modal-head" }).range(at, at + 1)]);
}

const MODE_LABEL = { normal: "NOR", insert: "INS", select: "SEL" } as const;

function bar(view: EditorView): Panel {
  const dom = document.createElement("div");
  dom.className = "cm-modal-bar";
  const render = () => {
    const m = modals.get(view);
    if (!m) return;
    dom.replaceChildren();
    const mode = document.createElement("span");
    mode.className = `cm-modal-mode ${m.keys.mode}`;
    mode.textContent = MODE_LABEL[m.keys.mode];
    dom.append(mode);
    if (m.line) {
      const input = document.createElement("input");
      input.className = "cm-modal-line";
      const prefix = document.createElement("span");
      prefix.textContent = m.line.search ? "/" : ":";
      dom.append(prefix, input);
      const search = m.line.search;
      input.addEventListener("keydown", (e) => {
        if (e.key !== "Enter" && e.key !== "Escape") return;
        e.preventDefault();
        m.line = undefined;
        view.focus();
        if (e.key === "Enter" && input.value) {
          sync(view, m);
          if (search) act(view, m, m.keys.searchFor(m.buf, input.value));
          else command(view, m, input.value);
        }
        view.dispatch({ effects: tick.of(null) });
      });
      requestAnimationFrame(() => input.focus());
      return;
    }
    const pending = m.keys.pending();
    if (pending) {
      const keys = document.createElement("span");
      keys.className = "cm-modal-pending";
      keys.textContent = pending;
      dom.append(keys);
      for (const [key, label] of after(pending, m.keys.mode === "select")) {
        const item = document.createElement("span");
        item.className = "cm-modal-item";
        const k = document.createElement("b");
        k.textContent = key;
        item.append(k, ` ${label}`);
        dom.append(item);
      }
    }
    if (m.notice) {
      const notice = document.createElement("span");
      notice.className = "cm-modal-notice";
      notice.textContent = m.notice;
      dom.append(notice);
    }
  };
  render();
  return {
    dom,
    update(u: ViewUpdate) {
      if (u.transactions.some((t) => t.effects.some((e) => e.is(tick))) || u.selectionSet) {
        const m = modals.get(view);
        // The line keeps its input while it's open.
        if (!m?.line || !dom.querySelector("input")) render();
      }
    },
  };
}

const theme = EditorView.theme({
  "&.cm-modal-on.cm-modal-normal .cm-cursor, &.cm-modal-on.cm-modal-select .cm-cursor": {
    display: "none",
  },
  ".cm-modal-head": { background: "var(--fg)", color: "var(--bg)" },
  ".cm-modal-eol": {
    display: "inline-block",
    width: "1ch",
    height: "1.2em",
    verticalAlign: "text-bottom",
    background: "var(--fg)",
  },
  ".cm-modal-bar": {
    display: "flex",
    alignItems: "center",
    gap: "10px",
    padding: "2px 8px",
    font: "12px var(--mono)",
    minHeight: "20px",
  },
  ".cm-modal-mode": { fontWeight: "600", color: "var(--muted)" },
  ".cm-modal-mode.insert": { color: "var(--accent)" },
  ".cm-modal-mode.select": { color: "var(--warning, #d7a14a)" },
  ".cm-modal-item b": { color: "var(--accent)", fontWeight: "600" },
  ".cm-modal-notice": { color: "var(--error)" },
  ".cm-modal-line": {
    flex: "1",
    background: "transparent",
    border: "none",
    outline: "none",
    color: "var(--fg)",
    font: "inherit",
  },
});

/** Modal editing for the file at `path`. It follows `modalEditing`. */
export function modal(path: string, hooks: ModalHooks): Extension {
  const plugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      stop: () => boolean;
      constructor(readonly view: EditorView) {
        const m = new Modal(path, hooks);
        modals.set(view, m);
        sync(view, m);
        this.decorations = cursorDeco(view);
        this.classes();
        this.stop = modalEditing.subscribe(() => {
          const on = modalEditing.get();
          if (on === view.state.field(onField)) return;
          if (!on) m.keys = new Keys(hooks.readOnly);
          view.dispatch({ effects: onEffect.of(on) });
        });
      }
      update(u: ViewUpdate) {
        this.decorations = cursorDeco(u.view);
        this.classes();
      }
      classes() {
        const m = modals.get(this.view);
        const on = this.view.state.field(onField);
        const cls = this.view.dom.classList;
        cls.toggle("cm-modal-on", on);
        for (const mode of ["normal", "insert", "select"] as const) {
          cls.toggle(`cm-modal-${mode}`, on && m?.keys.mode === mode);
        }
      }
      destroy() {
        this.stop();
      }
    },
    { decorations: (p) => p.decorations },
  );
  return [
    onField,
    codeFolding(),
    plugin,
    showPanel.from(onField, (on) => (on ? bar : null)),
    Prec.highest(EditorView.domEventHandlers({ keydown: onKey })),
    theme,
  ];
}
