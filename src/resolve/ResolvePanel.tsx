import type { IDockviewPanelProps } from "dockview-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ConflictView, FileChoice, ResolveAct, ResolveView } from "../bindings";
import { isMac } from "../core/keys";
import { useWorkspace } from "../core/workspace";
import { baseName, type EditorParams, openFile } from "../editor/open";
import { setUnsaved } from "../editor/unsaved";
import { commands, errorText, unwrap } from "../ipc";
import { Block, SideLabel } from "./Block";
import { paint, type Segment } from "./paint";
import "../review/syntax.css";
import "./resolve.css";

const conflicts = (v: ResolveView) =>
  v.parts.flatMap((p) => (p.kind === "conflict" ? [p as ConflictView] : []));

/** What the file bar offers for a file a side deleted or moved. */
function choices(v: ResolveView): [string, FileChoice][] {
  switch (v.fate.kind) {
    case "deleted":
      return [
        ["Keep file", "keep"],
        ["Delete file", "delete"],
      ];
    case "moved":
      return [
        [`Carry edits to ${v.fate.to}`, "carry"],
        ["Keep both", "both"],
        ["Drop edits", "drop"],
      ];
    default:
      return [];
  }
}

/** Keys for the file bar's choices. */
const CHOICE_KEYS: Record<string, FileChoice> = {
  F: "keep",
  D: "delete",
  c: "carry",
  b: "both",
  x: "drop",
};

/** A working-copy file's conflicts, resolved side by side: numbers keep sides in the order
 * pressed, kept sides drag or move with J and K, and Save writes what's resolved. */
export function ResolvePanel({ params, api }: IDockviewPanelProps<EditorParams>) {
  const workspace = useWorkspace();
  const { path } = params;
  const [view, setView] = useState<ResolveView>();
  const [error, setError] = useState<string>();
  /** The conflict keys act on, and its kept side. */
  const [current, setCurrent] = useState(0);
  const [pick, setPick] = useState<number>();
  const [editing, setEditing] = useState<number>();
  const colors = useRef(new Map<string, Segment[][]>());
  const [, setColored] = useState(0);
  const root = useRef<HTMLDivElement>(null);

  const open = useCallback(
    (fresh: boolean) =>
      unwrap(commands.resolveOpen(workspace, path, fresh)).then(
        (v) => {
          setView(v);
          setError(undefined);
          const first = conflicts(v).findIndex((c) => !c.resolved);
          setCurrent(Math.max(first, 0));
          setPick(undefined);
        },
        (e) => setError(errorText(e)),
      ),
    [workspace, path],
  );

  useEffect(() => {
    open(false);
    return () => void commands.resolveClose(workspace, path);
  }, [open, workspace, path]);

  // Highlighting for each text shown, asked for once.
  useEffect(() => {
    if (!view) return;
    const texts = new Set<string>();
    for (const p of view.parts) {
      if (p.kind === "text") texts.add(p.text);
      else for (const s of [...p.picks, ...p.rest]) texts.add(s.text);
    }
    for (const t of texts) {
      if (colors.current.has(t)) continue;
      colors.current.set(t, []);
      commands.highlightText(path, t).then((hl) => {
        colors.current.set(t, hl);
        setColored((n) => n + 1);
      });
    }
  }, [view, path]);

  async function act(a: ResolveAct) {
    try {
      setView(await unwrap(commands.resolveAct(workspace, path, a)));
      setError(undefined);
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function save(): Promise<boolean> {
    try {
      const { view: v, saved } = await unwrap(commands.resolveSave(workspace, path));
      setView(v);
      setError(undefined);
      if (saved.clash) openFile(workspace, saved.clash);
      if (saved.removed) api.close();
      return true;
    } catch (e) {
      setError(errorText(e));
      return false;
    }
  }
  const saveRef = useRef(save);
  saveRef.current = save;

  const dirty = !!view?.dirty;
  useEffect(() => {
    api.setTitle(dirty ? `● ${baseName(path)}` : baseName(path));
    const name = workspace ? `${path} (${workspace})` : path;
    setUnsaved(workspace, api.id, dirty ? { name, save: () => saveRef.current() } : undefined);
  }, [api, workspace, path, dirty]);
  useEffect(() => () => setUnsaved(workspace, api.id, undefined), [api, workspace]);

  useEffect(() => {
    const sub = api.onDidActiveChange((e) => e.isActive && root.current?.focus());
    root.current?.focus();
    return () => sub.dispose();
  }, [api]);

  const list = view ? conflicts(view) : [];

  function goTo(i: number) {
    setCurrent(i);
    setPick(undefined);
    root.current
      ?.querySelector(`[data-conflict="${i}"]`)
      ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (!view || editing !== undefined) return;
    const mod = isMac ? e.metaKey : e.ctrlKey;
    const c = list[current];
    const key = e.key;
    let done = true;
    if (mod && key.toLowerCase() === "s") void save();
    else if (mod && key.toLowerCase() === "z") void act({ kind: e.shiftKey ? "redo" : "undo" });
    else if (mod || e.altKey) done = false;
    else if (/^[0-9]$/.test(key) && c) void act({ kind: "toggle", conflict: c.index, key: +key });
    else if (key === "n" && list.length) goTo((current + 1) % list.length);
    else if (key === "N" && list.length) goTo((current - 1 + list.length) % list.length);
    else if ((key === "J" || key === "K") && c && pick !== undefined) {
      const to = key === "J" ? pick + 1 : pick - 1;
      if (to >= 0 && to < c.picks.length) {
        void act({ kind: "place", conflict: c.index, from: pick, to });
        setPick(to);
      }
    } else if (key === "o" && c && pick !== undefined)
      void act({ kind: "flip", conflict: c.index, pick });
    else if (key === "i" && c && pick !== undefined) setEditing(pick);
    else if (key === "u") void act({ kind: "undo" });
    else if (key === "U") void act({ kind: "redo" });
    else if (CHOICE_KEYS[key]) void act({ kind: "choose", choice: CHOICE_KEYS[key] });
    else done = false;
    if (done) e.preventDefault();
  }

  if (!view) {
    return <div className="rz">{error && <div className="editor-bar error">{error}</div>}</div>;
  }
  const slash = path.lastIndexOf("/");
  const hl = (text: string) => colors.current.get(text);
  const bar = choices(view);

  return (
    // biome-ignore lint/a11y/noNoninteractiveTabindex: focusable so its keys work
    <div className="rz" ref={root} tabIndex={0} onKeyDown={onKeyDown}>
      <div className="rz-bar">
        <span className="rz-status">C</span>
        <span className="path" title={path}>
          <span className="dir">{path.slice(0, slash + 1)}</span>
          {path.slice(slash + 1)}
        </span>
        <span className={`pill ${view.done ? "green" : "amber"}`}>
          {view.done ? "Resolved" : `${view.resolved} of ${view.total} resolved`}
        </span>
        <span className="spacer" />
        <button type="button" disabled={!view.dirty} onClick={() => void open(true)}>
          Discard
        </button>
        <button
          type="button"
          className="primary"
          disabled={!view.dirty}
          onClick={() => void save()}
        >
          Save
        </button>
      </div>
      {error && <div className="editor-bar error">{error}</div>}
      <div className="rz-body">
        {view.fate.kind !== "here" && (
          <div className={`rz-file${view.choice ? " done" : ""}`}>
            <span>{view.fate.kind === "moved" ? `Moved to ${view.fate.to} in` : "Deleted in"}</span>
            <SideLabel label={view.fate.by} />
            <span className="spacer" />
            {bar.map(([label, choice]) => (
              <button
                key={choice}
                type="button"
                aria-pressed={view.choice === choice}
                onClick={() => void act({ kind: "choose", choice })}
              >
                {label}
              </button>
            ))}
          </div>
        )}
        {view.parts.map((p) => {
          if (p.kind === "text") {
            const lines = p.text.split("\n");
            if (lines[lines.length - 1] === "") lines.pop();
            const segs = hl(p.text);
            return (
              <div key={`t${p.line}`} className="rz-lines">
                {lines.map((text, n) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: lines are by place
                  <div key={n} className="rz-line">
                    <span className="ln">{p.line + n}</span>
                    <span className="code">{paint(text, segs?.[n] ?? [], [])}</span>
                  </div>
                ))}
              </div>
            );
          }
          const c = p as ConflictView;
          const i = list.indexOf(c);
          return (
            <div key={`c${c.index}`} data-conflict={i}>
              <Block
                conflict={c}
                total={view.total}
                current={i === current}
                pick={i === current ? pick : undefined}
                editing={i === current ? editing : undefined}
                removing={view.removing}
                hl={hl}
                select={(at) => {
                  setCurrent(i);
                  setPick(at);
                  root.current?.focus();
                }}
                toggle={(key) => void act({ kind: "toggle", conflict: c.index, key })}
                place={(from, to) => void act({ kind: "place", conflict: c.index, from, to })}
                flip={(at) => void act({ kind: "flip", conflict: c.index, pick: at })}
                edit={(at) => {
                  setCurrent(i);
                  setEditing(at);
                  if (at === undefined) root.current?.focus();
                }}
                setText={(at, text) =>
                  void act({ kind: "edit", conflict: c.index, pick: at, text })
                }
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
