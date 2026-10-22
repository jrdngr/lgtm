import { EditorState, type Text, Transaction } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useRef, useState } from "react";
import { isMac } from "../core/keys";
import { useWorkspace } from "../core/workspace";
import { FindBar } from "../find/FindBar";
import { useCmFind } from "../find/useCmFind";
import { useDomFind } from "../find/useDomFind";
import { commands, errorText, unwrap } from "../ipc";
import { useSendText } from "../review/useSendText";
import { showInfos } from "../symbolInfo/editorInfo";
import { useFolded, useSymbolInfos } from "../symbolInfo/store";
import { type Sender, sendItems } from "../terminal/sendItems";
import { copy, openMenu } from "../ui/contextMenu";
import { saveTabParams } from "../workbench/tabParams";
import { wordWrap } from "../workbench/wordWrap";
import { useRepoChanged } from "../workspaces/useRepoChanged";
import { anchorLine } from "./anchors";
import { minimalChange } from "./change";
import { editorMenu, pathAndLine, placeAt } from "./editorMenu";
import { HtmlView } from "./HtmlView";
import { isHtml } from "./htmlNav";
import { ImagePanel } from "./ImagePanel";
import { imageType, isSvg, svgUrl } from "./image";
import { followLink } from "./links";
import { MarkdownView } from "./MarkdownView";
import { baseName, type EditorParams } from "./open";
import { lineRef, renderedLines } from "./renderedLines";
import { editorExtensions, wrapExtension, wrapping } from "./setup";
import { setUnsaved } from "./unsaved";
import { useImageZoom } from "./useImageZoom";
import { ViewSwitch } from "./ViewSwitch";
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
  const { path, line, anchor, nonce } = params;
  const host = useRef<HTMLDivElement>(null);
  const [view, setView] = useState<EditorView>();
  /** The text as last read or saved. */
  const saved = useRef<Text>(undefined);
  const [dirty, setDirty] = useState(false);
  /** Newer text on disk, held back because of unsaved edits. */
  const [onDisk, setOnDisk] = useState<string>();
  const [error, setError] = useState<string>();
  /** How the tab was left, for bringing it back after a restart. */
  const restore = useRef(params);
  /** The text shown rendered, or undefined while editing. HTML opens rendered. */
  const [rendered, setRendered] = useState(
    (params.view ?? (isHtml(path) ? "rendered" : "editor")) === "rendered" ? "" : undefined,
  );
  const renderedRef = useRef<{ focus(): void }>(null);
  const saveRef = useRef(async () => false);
  const wrap = wordWrap.use();
  const cmFind = useCmFind(view);
  const domFind = useDomFind(renderedRef as React.RefObject<HTMLElement | null>);
  const findRef = useRef(cmFind);
  findRef.current = cmFind;
  const { sendRecent, sendToId } = useSendText(workspace, setError, () => view?.focus());

  useEffect(() => {
    let live = true;
    let made: EditorView | undefined;
    unwrap(commands.readFile(workspace, path)).then(
      (text) => {
        if (!live || !host.current) return;
        const extensions = editorExtensions(path, text, wordWrap.get(), {
          save: () => saveRef.current(),
          link: (href) => followLink(href, workspace, path),
          find: () => findRef.current.show(),
          changed: (u) => {
            findRef.current.changed(u);
            if (u.selectionSet) saveTabParams(api, { cursor: u.state.selection.main.head });
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
        setRendered((r) => (r === undefined ? r : text));
        const { cursor, scroll } = restore.current;
        if (cursor !== undefined && !restore.current.line) {
          made.dispatch({ selection: { anchor: Math.min(cursor, made.state.doc.length) } });
        }
        const scroller = made.scrollDOM;
        if (scroll) requestAnimationFrame(() => scroller.scrollTo({ top: scroll }));
        scroller.addEventListener("scroll", () =>
          saveTabParams(api, { scroll: Math.round(scroller.scrollTop) }),
        );
        setView(made);
      },
      (e) => live && setError(errorText(e)),
    );
    return () => {
      live = false;
      made?.destroy();
    };
  }, [api, workspace, path]);

  // Rendered markdown goes back to where it was scrolled, once, when its text first shows.
  useEffect(() => {
    const top = restore.current.renderedScroll;
    if (!rendered || !top) return;
    restore.current = { ...restore.current, renderedScroll: undefined };
    const el = renderedRef.current;
    requestAnimationFrame(() => el instanceof HTMLElement && el.scrollTo({ top }));
  }, [rendered]);

  useEffect(() => {
    view?.dispatch({ effects: wrapping.reconfigure(wrapExtension(wrap)) });
  }, [view, wrap]);

  const infos = useSymbolInfos(workspace, [path]);
  const folded = useFolded();
  useEffect(() => {
    view?.dispatch({ effects: showInfos.of({ ws: workspace, infos, folded }) });
  }, [view, workspace, infos, folded]);

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
    // Used: a restart brings back the cursor where it was left, not this line.
    saveTabParams(api, { line: undefined, nonce: undefined });
  }, [api, view, line, nonce]);

  // A link's anchor shows where it points, in whichever view the tab is in.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `nonce` follows the same link again
  useEffect(() => {
    if (!view || !anchor) return;
    saveTabParams(api, { anchor: undefined, nonce: undefined });
    const at = anchorLine(view.state.doc.toString(), anchor);
    if (at === undefined) return;
    const pos = view.state.doc.line(Math.min(at, view.state.doc.lines)).from;
    view.dispatch({
      selection: { anchor: pos },
      effects: EditorView.scrollIntoView(pos, { y: "start", yMargin: 24 }),
    });
    if (renderedRef.current instanceof HTMLElement) {
      const root = renderedRef.current;
      requestAnimationFrame(() => {
        const blocks = [...root.querySelectorAll<HTMLElement>("[data-start]")];
        blocks.find((b) => Number(b.dataset.end) >= at)?.scrollIntoView({ block: "start" });
      });
    } else view.focus();
  }, [api, view, anchor, nonce]);

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
      return true;
    } catch (e) {
      setError(errorText(e));
      return false;
    }
  }
  saveRef.current = save;

  async function sync() {
    if (!view || !saved.current) return;
    // A file that went away keeps its editor, to save elsewhere by copying.
    const text = await unwrap(commands.readFile(workspace, path)).catch(() => undefined);
    if (text === undefined || view.state.toText(text).eq(saved.current)) return;
    if (view.state.doc.eq(saved.current)) load(view, text);
    else setOnDisk(text);
  }
  useRepoChanged(sync);

  function toggleRendered() {
    if (!view) return;
    saveTabParams(api, { view: rendered === undefined ? "rendered" : "editor" });
    if (rendered === undefined) {
      setRendered(view.state.sliceDoc());
      requestAnimationFrame(() => renderedRef.current?.focus());
    } else {
      setRendered(undefined);
      requestAnimationFrame(() => view.focus());
    }
  }

  /** Where the selection is, as `path:lines`: in the rendered markdown when it shows (or the
   * block at `at` there), else in the text. */
  function selectionRef(at?: Node): string {
    const root = renderedRef.current;
    if (root instanceof HTMLElement && markdown) {
      const lines = renderedLines(root, at);
      return lines ? lineRef(path, lines) : path;
    }
    return view ? pathAndLine(view, path) : path;
  }

  const refocus = () => (renderedRef.current ?? view)?.focus();
  const sender: Sender = {
    recent: (text) => sendRecent(text, refocus),
    to: (text, id) => sendToId(text, id, refocus),
  };

  function onKeyDown(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (mod && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "f" && !html) {
      e.preventDefault();
      if (rendered === undefined) cmFind.show();
      else if (markdown) domFind.show();
    } else if (mod && e.shiftKey && e.key.toLowerCase() === "v" && renders(path)) {
      e.preventDefault();
      toggleRendered();
    } else if (mod && e.shiftKey && (e.key === ">" || e.key === ".") && !html) {
      e.preventDefault();
      sender.recent(selectionRef());
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
        send: sender,
        toggleRendered: renders(path) ? toggleRendered : undefined,
      }),
    );
  }

  function onRenderedMenu(e: React.MouseEvent) {
    const text = window.getSelection()?.toString() ?? "";
    openMenu(e, [
      { label: "Copy", keys: "⌘C", disabled: !text, onSelect: () => copy(text) },
      "-",
      ...sendItems(workspace, "Send", selectionRef(e.target as Node), sender),
      "-",
      { label: "Show Editor", keys: "⇧⌘V", onSelect: toggleRendered },
    ]);
  }

  function keepMine() {
    if (!view || onDisk === undefined) return;
    saved.current = view.state.toText(onDisk);
    setDirty(!view.state.doc.eq(saved.current));
    setOnDisk(undefined);
  }

  const toggle = renders(path) && (
    <ViewSwitch rendered={rendered !== undefined} disabled={!view} toggle={toggleRendered} />
  );
  const html = isHtml(path) && rendered !== undefined;
  const markdown = rendered !== undefined && !html && !isSvg(path);
  const svg = useImageZoom(
    isSvg(path) && rendered !== undefined ? svgUrl(rendered) : undefined,
    path,
    restore.current.zoom,
    (zoom) => saveTabParams(api, { zoom: zoom === "shrink" ? undefined : zoom }),
  );

  return (
    <div className="editor-panel" onKeyDown={onKeyDown}>
      {!html && (
        <div className="editor-path">
          <span className="path" title={path}>
            {path}
          </span>
          {svg.view && svg.controls}
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
      {!html && (rendered === undefined ? cmFind.open : markdown && domFind.open) && (
        <div className="find-anchor">
          <FindBar {...(rendered === undefined ? cmFind.bar : domFind.bar)} />
        </div>
      )}
      {html && (
        <HtmlView
          ref={renderedRef}
          workspace={workspace}
          path={path}
          toggle={toggle}
          saved={restore.current}
          save={(values) => saveTabParams(api, values)}
        />
      )}
      {rendered !== undefined && !html && (
        <div
          className={`editor-rendered${isSvg(path) ? " image" : ""}`}
          ref={renderedRef as React.RefObject<HTMLDivElement>}
          // biome-ignore lint/a11y/noNoninteractiveTabindex: focusable so ⇧⌘V and scrolling keys work
          tabIndex={0}
          onKeyDown={svg.onKeyDown}
          onContextMenu={markdown ? onRenderedMenu : undefined}
          onScroll={(e) =>
            saveTabParams(api, { renderedScroll: Math.round(e.currentTarget.scrollTop) })
          }
        >
          {isSvg(path) ? (
            svg.view
          ) : (
            <MarkdownView
              text={rendered}
              lines
              onLink={(href) => followLink(href, workspace, path)}
            />
          )}
        </div>
      )}
      <div
        className="editor-host"
        ref={host}
        hidden={rendered !== undefined}
        onContextMenu={onContextMenu}
      />
    </div>
  );
}
