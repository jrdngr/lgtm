// @vitest-environment jsdom
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { afterEach, describe, expect, it } from "vitest";
import { modal, modalEditing } from "./cm";

let view: EditorView;

function editor(text: string, at = 0) {
  view = new EditorView({
    parent: document.body,
    state: EditorState.create({
      doc: text,
      selection: { anchor: at },
      extensions: modal("t.txt", { save: () => {} }),
    }),
  });
  return view;
}

function press(keys: string) {
  for (const key of keys.match(/<[^>]+>|./g) ?? []) {
    const name = key === "<esc>" ? "Escape" : key;
    view.contentDOM.dispatchEvent(new KeyboardEvent("keydown", { key: name, bubbles: true }));
  }
}

const type = (text: string) => view.dispatch(view.state.replaceSelection(text));
const sel = () => [view.state.selection.main.anchor, view.state.selection.main.head];

afterEach(() => {
  view.destroy();
  modalEditing.set(true);
});

describe("modal editing in CodeMirror", () => {
  it("selects the char under the cursor and moves by words", () => {
    editor("let argv = 1", 4);
    press("w");
    expect(sel()).toEqual([4, 9]);
  });

  it("deletes with d and takes it back with u", () => {
    editor("abc", 1);
    press("d");
    expect(view.state.doc.toString()).toBe("ac");
    press("u");
    expect(view.state.doc.toString()).toBe("abc");
  });

  it("lets CodeMirror type in insert mode and leaves it on Escape", () => {
    editor("abc", 1);
    press("i");
    expect(sel()).toEqual([1, 1]);
    type("XY");
    press("<esc>");
    expect(view.state.doc.toString()).toBe("aXYbc");
    expect(sel()).toEqual([2, 3]);
    press("u");
    expect(view.state.doc.toString()).toBe("abc");
  });

  it("drops its bar and leaves keys to CodeMirror while modal editing is off", () => {
    editor("abc", 1);
    expect(view.dom.querySelector(".cm-modal-bar")).not.toBeNull();
    modalEditing.set(false);
    expect(view.dom.querySelector(".cm-modal-bar")).toBeNull();
    const e = new KeyboardEvent("keydown", { key: "d", bubbles: true, cancelable: true });
    view.contentDOM.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
    expect(view.state.doc.toString()).toBe("abc");
  });
});
