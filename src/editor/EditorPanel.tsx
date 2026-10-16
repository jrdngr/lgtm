import { EditorState, type Text, Transaction } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useRef, useState } from "react";
import { commands, errorText, unwrap } from "../ipc";
import { useSendText } from "../review/useSendText";
import { openMenu } from "../ui/contextMenu";
import { isMac } from "../workbench/keys";
import { wordWrap } from "../workbench/wordWrap";
import { useWorkspace } from "../workspaces/context";
import { useRepoChanged } from "../workspaces/useRepoChanged";
import type { Reload } from "./AgentBox";
import { minimalChange } from "./change";
import { editorMenu, placeAt } from "./editorMenu";
import { HtmlView } from "./HtmlView";
import { isHtml } from "./htmlNav";
import { ImagePanel } from "./ImagePanel";
import { imageType, isSvg, svgUrl } from "./image";
import { MarkdownView } from "./MarkdownView";
import { baseName, type EditorParams } from "./open";
import { emitSaved } from "./saved";
import { editorExtensions, wrapExtension, wrapping } from "./setup";
import { setUnsaved } from "./unsaved";
import { useAgentBox } from "./useAgentBox";
import "./editor.css";

const isMarkdown = (path: string) => /\.(md|markdown)$/i.test(path);
/** Files with a rendered view next to their text. */
const renders = (path: string) => isMarkdown(path) || isSvg(path) || isHtml(path);

/** A working-copy file in its tab: an image view for images, else the text editor. */
export function EditorPanel(props: IDockviewPanelProps<EditorParams>) {
  return imageType(props.params.path) ? <ImagePanel {...props} /> : <TextPanel {...props} />;
}

/** A working-copy file in CodeMirror. Saving snapshots the working copy, so reviews of `@`
 * update. Changes on disk load in place, or wait behind a bar while there are unsaved edits. */
function TextPanel({ params, api }: IDockviewPanelProps<EditorParams>) {
  const workspace = useWorkspace();
  const { path, line, nonce } = params;
  const host = useRef<HTMLDivElement>(null);
  const [view, setView] = useState<EditorView>();
  /** The text as last read or saved. */
  const saved = useRef<Text>(undefined);
  const [dirty, setDirty] = useState(false);
  /** Newer text on disk, held back because of unsaved edits. */
  const [onDisk, setOnDisk] = useState<string>();
  const [error, setError] = useState<string>();
  /** The text shown rendered, or undefined while editing. HTML opens rendered. */
  const [rendered, setRendered] = useState(isHtml(path) ? "" : undefined);
  const renderedRef = useRef<{ focus(): void }>(null);
  const saveRef = useRef(async () => false);
  const wrap = wordWrap.use();
  const { sendText, picker } = useSendText(workspace, setError, () => view?.focus());

  useEffect(() => {
    let live = true;
    let made: EditorView | undefined;
    unwrap(commands.readFile(workspace, path)).then(
      (text) => {
        if (!live || !host.current) return;
        const extensions = editorExtensions(path, text, wordWrap.get(), {
          save: () => saveRef.current(),
          changed: (u) => {
            if (!u.docChanged) return;
            if (saved.current) setDirty(!u.state.doc.eq(saved.current));
            // A reload from disk shows in the rendered view too.
            setRendered((r) => (r === undefined ? r : u.state.sliceDoc()));
          },
        });
        made = new EditorView({
          parent: host.current,
          state: EditorState.create({ doc: text, extensions }),
        });
        saved.current = made.state.doc;
        setView(made);
      },
      (e) => live && setError(errorText(e)),
    );
    return () => {
      live = false;
      made?.destroy();
    };
  }, [workspace, path]);

  useEffect(() => {
    view?.dispatch({ effects: wrapping.reconfigure(wrapExtension(wrap)) });
  }, [view, wrap]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: `nonce` reopens at the same line
  useEffect(() => {
    if (!view || !line) return;
    const doc = view.state.doc;
    const at = doc.line(Math.min(Math.max(line, 1), doc.lines)).from;
    setRendered(undefined);
    view.dispatch({
      selection: { anchor: at },
      effects: EditorView.scrollIntoView(at, { y: "center" }),
    });
    view.focus();
  }, [view, line, nonce]);

  useEffect(() => {
    api.setTitle(dirty ? `● ${baseName(path)}` : baseName(path));
    const name = workspace ? `${path} (${workspace})` : path;
    setUnsaved(workspace, api.id, dirty ? { name, save: () => saveRef.current() } : undefined);
  }, [api, workspace, path, dirty]);

  useEffect(() => () => setUnsaved(workspace, api.id, undefined), [api, workspace]);

  useEffect(() => {
    const sub = api.onDidActiveChange((e) => {
      if (e.isActive) (renderedRef.current ?? view)?.focus();
    });
    return () => sub.dispose();
  }, [api, view]);

  /** Puts `text` in the editor as the saved text, keeping the cursor where it can. */
  function load(v: EditorView, text: string) {
    const change = minimalChange(v.state.sliceDoc(), text);
    v.dispatch({ changes: change, annotations: Transaction.addToHistory.of(false) });
    saved.current = v.state.doc;
    setDirty(false);
    setOnDisk(undefined);
    setError(undefined);
  }

  async function save(): Promise<boolean> {
    if (!view) return false;
    const doc = view.state.doc;
    try {
      await unwrap(commands.saveFile(workspace, path, view.state.sliceDoc()));
      saved.current = doc;
      setDirty(!view.state.doc.eq(doc));
      setOnDisk(undefined);
      setError(undefined);
      emitSaved(path);
      return true;
    } catch (e) {
      setError(errorText(e));
      return false;
    }
  }
  saveRef.current = save;

  async function sync(): Promise<Reload> {
    if (!view || !saved.current) return "same";
    // A file that went away keeps its editor, to save elsewhere by copying.
    const text = await unwrap(commands.readFile(workspace, path)).catch(() => undefined);
    if (text === undefined || view.state.toText(text).eq(saved.current)) return "same";
    if (view.state.doc.eq(saved.current)) {
      load(view, text);
      return "loaded";
    }
    setOnDisk(text);
    return "held";
  }
  useRepoChanged(sync);
  const agent = useAgentBox(workspace, path, view, sync, () => api.setActive());

  function toggleRendered() {
    if (!view) return;
    if (rendered === undefined) {
      setRendered(view.state.sliceDoc());
      requestAnimationFrame(() => renderedRef.current?.focus());
    } else {
      setRendered(undefined);
      requestAnimationFrame(() => view.focus());
    }
  }

  function onKeyDown(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (e.key === "Escape" && agent.isOpen && !e.defaultPrevented) {
      e.preventDefault();
      agent.close();
    }
    if (mod && e.shiftKey && e.key.toLowerCase() === "v" && renders(path)) {
      e.preventDefault();
      toggleRendered();
    }
  }

  function onContextMenu(e: React.MouseEvent) {
    if (!view) return;
    placeAt(view, e);
    openMenu(
      e,
      editorMenu({
        view,
        workspace,
        path,
        dirty,
        save: () => void save(),
        sendText: (text) => sendText(text, () => view.focus()),
        askAgent: agent.open,
        toggleRendered: renders(path) ? toggleRendered : undefined,
      }),
    );
  }

  function keepMine() {
    if (!view || onDisk === undefined) return;
    saved.current = view.state.toText(onDisk);
    setDirty(!view.state.doc.eq(saved.current));
    setOnDisk(undefined);
  }

  const toggle = renders(path) && (
    <button type="button" disabled={!view} onClick={toggleRendered} title="⇧⌘V">
      {rendered === undefined ? "Rendered" : "Editor"}
    </button>
  );
  const html = isHtml(path) && rendered !== undefined;

  return (
    <div className="editor-panel" onKeyDown={onKeyDown}>
      {!html && (
        <div className="editor-path">
          <span className="path" title={path}>
            {path}
          </span>
          {toggle}
        </div>
      )}
      {error && <div className="editor-bar error">{error}</div>}
      {onDisk !== undefined && (
        <div className="editor-bar">
          <span>Changed on disk while you have unsaved edits.</span>
          <button type="button" onClick={() => view && load(view, onDisk)}>
            Load theirs
          </button>
          <button type="button" onClick={keepMine}>
            Keep mine
          </button>
        </div>
      )}
      {html && <HtmlView ref={renderedRef} workspace={workspace} path={path} toggle={toggle} />}
      {rendered !== undefined && !html && (
        <div
          className={`editor-rendered${isSvg(path) ? " image" : ""}`}
          ref={renderedRef as React.RefObject<HTMLDivElement>}
          // biome-ignore lint/a11y/noNoninteractiveTabindex: focusable so ⇧⌘V and scrolling keys work
          tabIndex={0}
        >
          {isSvg(path) ? (
            <div className="image-view">
              <img src={svgUrl(rendered)} alt={path} draggable={false} />
            </div>
          ) : (
            <MarkdownView text={rendered} />
          )}
        </div>
      )}
      <div
        className="editor-host"
        ref={host}
        hidden={rendered !== undefined}
        onContextMenu={onContextMenu}
      />
      {agent.portal}
      {picker}
    </div>
  );
}
