import { EditorState, type Text, Transaction } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useRef, useState } from "react";
import type { NoteDto } from "../bindings";
import { isMac } from "../core/keys";
import { useWorkspace } from "../core/workspace";
import { minimalChange } from "../editor/change";
import { followLink } from "../editor/links";
import { MarkdownView } from "../editor/MarkdownView";
import { editorExtensions, wrapExtension, wrapping } from "../editor/setup";
import { ViewSwitch } from "../editor/ViewSwitch";
import { commands, errorText, unwrap } from "../ipc";
import { saveTabParams } from "../workbench/tabParams";
import { wordWrap } from "../workbench/wordWrap";
import { useNote } from "./store";
import "../editor/editor.css";

interface Params {
  note: string;
  view?: "editor" | "rendered";
}

const LEVELS = { global: "Global", repo: "Repo", workspace: "Workspace" } as const;

/** Where a note sits, like `Repo / Plans / Tokens`. */
const crumb = (n: NoteDto) =>
  [LEVELS[n.level], ...n.folder.split("/").filter(Boolean), n.name].join(" / ");

/** How long typing pauses before the note saves. */
const SAVE_AFTER = 400;

/** A note in its own tab: the markdown editor and its rendered view, saving as you type. */
export function NoteTab({ api, params }: IDockviewPanelProps<Params>) {
  const workspace = useWorkspace();
  const { note, error: loadError } = useNote(workspace, params.note);
  const host = useRef<HTMLDivElement>(null);
  const [view, setView] = useState<EditorView>();
  /** The text as last loaded or saved. */
  const saved = useRef<Text>(undefined);
  const timer = useRef<number>(undefined);
  const [error, setError] = useState<string>();
  const [rendered, setRendered] = useState<string | undefined>(
    params.view === "editor" ? undefined : "",
  );
  const renderedRef = useRef<HTMLDivElement>(null);
  const wrap = wordWrap.use();
  const id = params.note;
  const body = note?.body;

  async function save(v: EditorView) {
    window.clearTimeout(timer.current);
    timer.current = undefined;
    const doc = v.state.doc;
    if (saved.current && doc.eq(saved.current)) return;
    try {
      await unwrap(commands.saveNote(workspace, id, v.state.sliceDoc()));
      saved.current = doc;
      setError(undefined);
    } catch (e) {
      setError(errorText(e));
    }
  }
  const saveRef = useRef(save);
  saveRef.current = save;

  // The editor starts once the note first loads.
  const loaded = body !== undefined;
  // biome-ignore lint/correctness/useExhaustiveDependencies: made once, from the first load
  useEffect(() => {
    if (!loaded || !host.current) return;
    const text = body ?? "";
    const made = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: text,
        extensions: editorExtensions("note.md", text, wordWrap.get(), {
          save: () => saveRef.current(made),
          link: (href) => followLink(href, workspace),
          changed: (u) => {
            if (!u.docChanged) return;
            setRendered((r) => (r === undefined ? r : u.state.sliceDoc()));
            if (u.transactions.some((t) => t.annotation(Transaction.addToHistory) === false))
              return;
            window.clearTimeout(timer.current);
            timer.current = window.setTimeout(() => saveRef.current(made), SAVE_AFTER);
          },
        }),
      }),
    });
    saved.current = made.state.doc;
    setRendered((r) => (r === undefined ? r : text));
    setView(made);
    return () => {
      if (timer.current !== undefined) saveRef.current(made);
      made.destroy();
    };
  }, [loaded]);

  // Text changed elsewhere, by an agent or another window, loads unless you're mid-edit.
  useEffect(() => {
    if (!view || body === undefined || timer.current !== undefined) return;
    if (saved.current && view.state.toText(body).eq(saved.current)) return;
    view.dispatch({
      changes: minimalChange(view.state.sliceDoc(), body),
      annotations: Transaction.addToHistory.of(false),
    });
    saved.current = view.state.doc;
  }, [view, body]);

  useEffect(() => {
    view?.dispatch({ effects: wrapping.reconfigure(wrapExtension(wrap)) });
  }, [view, wrap]);

  useEffect(() => {
    if (note) api.setTitle(note.name);
  }, [api, note]);

  useEffect(() => {
    const sub = api.onDidActiveChange((e) => {
      if (e.isActive) (renderedRef.current ?? view)?.focus();
    });
    return () => sub.dispose();
  }, [api, view]);

  function toggleRendered() {
    if (!view) return;
    const show = rendered === undefined;
    saveTabParams(api, { view: show ? "rendered" : "editor" });
    setRendered(show ? view.state.sliceDoc() : undefined);
    requestAnimationFrame(() => (show ? renderedRef.current : view)?.focus());
  }

  function onKeyDown(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (mod && e.shiftKey && e.key.toLowerCase() === "v") {
      e.preventDefault();
      toggleRendered();
    }
  }

  return (
    <div className="editor-panel" onKeyDown={onKeyDown}>
      <div className="editor-path">
        <span className="path">{note ? crumb(note) : ""}</span>
        <ViewSwitch rendered={rendered !== undefined} disabled={!view} toggle={toggleRendered} />
      </div>
      {(error || loadError) && <div className="editor-bar error">{error || loadError}</div>}
      {rendered !== undefined && (
        <div
          className="editor-rendered"
          ref={renderedRef}
          // biome-ignore lint/a11y/noNoninteractiveTabindex: focusable so ⇧⌘V and scrolling keys work
          tabIndex={0}
        >
          <MarkdownView text={rendered} onLink={(href) => followLink(href, workspace)} />
        </div>
      )}
      <div className="editor-host" ref={host} hidden={rendered !== undefined} />
    </div>
  );
}
