import { useEffect, useMemo, useRef, useState } from "react";
import type { Hit, TextQuery } from "../bindings";
import { dockOf } from "../core/docks";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { commands, unwrap } from "../ipc";
import { currentReview, useCurrentReview } from "../review/current";
import { sendToTerminal } from "../terminal/registry";
import { confirmAction } from "../ui/prompt";
import { byFile, FileName, HitLine, hitsText } from "./HitLine";
import { type Scope, setText, type TextState } from "./store";
import { useHitKeys } from "./useHitKeys";

const SCOPES: [Scope, string][] = [
  ["workspace", "Workspace"],
  ["changed", "Changed files"],
  ["tabs", "Open tabs"],
];

/** The paths `scope` limits a search to, or null for the whole workspace. */
function scopePaths(ws: WorkspaceName, scope: Scope): string[] | null {
  if (scope === "changed") return currentReview(ws)?.files.map((f) => f.dto.path) ?? [];
  if (scope === "tabs") {
    const ids = dockOf(ws)?.panels.map((p) => p.id) ?? [];
    return ids.filter((id) => id.startsWith("editor:")).map((id) => id.slice(7));
  }
  return null;
}

interface Result {
  hits: Hit[];
  truncated: boolean;
  error?: string;
}

/** Text search across the workspace, with case, word and regex toggles, include and exclude
 * globs, a scope, and replace. */
export function TextView({ ws, text }: { ws: WorkspaceName; text: TextState }) {
  const { query: q, scope } = text;
  const review = useCurrentReview(ws);
  const changed = useMemo(() => {
    const out = new Set<string>();
    for (const f of review?.files ?? [])
      for (const r of f.rows) if (r.kind === "added") out.add(`${f.dto.path}:${r.newLine}`);
    return out;
  }, [review]);
  const [result, setResult] = useState<Result>({ hits: [], truncated: false });
  const [folded, setFolded] = useState<ReadonlySet<string>>(new Set());
  const [run, setRun] = useState(0);
  const boxRef = useRef<HTMLInputElement>(null);
  const update = (f: (q: TextQuery) => TextQuery) =>
    setText(ws, (t) => ({ ...t, query: f(t.query) }));

  useEffect(() => {
    if (text.asked) boxRef.current?.select();
  }, [text.asked]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: searches as the query changes
  useEffect(() => {
    if (!q.pattern) return setResult({ hits: [], truncated: false });
    let live = true;
    const t = setTimeout(() => {
      unwrap(commands.searchText(ws, { ...q, paths: scopePaths(ws, scope) })).then(
        (r) => live && setResult(r),
        (e) => live && setResult({ hits: [], truncated: false, error: String(e) }),
      );
    }, 200);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [ws, q, scope, run, text.asked]);

  // Kept between renders, since the hit cursor goes back to the top when they change.
  const shown = useMemo(
    () => result.hits.filter((h) => !folded.has(h.path)),
    [result.hits, folded],
  );
  const open = (h: Hit) => openFile(ws, h.path, h.line);

  async function replaceAll() {
    if (!q.pattern || !result.hits.length) return;
    const files = new Set(result.hits.map((h) => h.path)).size;
    const ok = await confirmAction(
      `Replace ${result.hits.length} matches in ${files} files?`,
      "Replace All",
    );
    if (!ok) return;
    unwrap(commands.replaceText(ws, { ...q, paths: scopePaths(ws, scope) }, text.replace)).then(
      () => setRun((n) => n + 1),
      (e) => setResult((r) => ({ ...r, error: String(e) })),
    );
  }

  const keys = useHitKeys(
    shown,
    open,
    () => sendToTerminal(ws, hitsText(result.hits)),
    (e) => {
      if (e.key !== "Enter" || !e.metaKey) return false;
      e.preventDefault();
      replaceAll();
      return true;
    },
  );
  const groups = byFile(result.hits);
  const toggle = (k: keyof TextQuery & ("regex" | "caseSensitive" | "wholeWord")) =>
    update((x) => ({ ...x, [k]: !x[k] }));

  return (
    <div className="sr-body" tabIndex={-1} onKeyDown={keys.onKeyDown}>
      <div className="sr-query">
        <div className="sr-field on">
          <input
            ref={boxRef}
            // biome-ignore lint/a11y/noAutofocus: the tab opens to type a search
            autoFocus
            spellCheck={false}
            value={q.pattern}
            onChange={(e) => update((x) => ({ ...x, pattern: e.target.value }))}
          />
          <Toggle on={q.caseSensitive} label="Aa" onClick={() => toggle("caseSensitive")} />
          <Toggle on={q.wholeWord} label="ab" onClick={() => toggle("wholeWord")} />
          <Toggle on={q.regex} label=".*" onClick={() => toggle("regex")} />
        </div>
        <div className="sr-field">
          <input
            spellCheck={false}
            placeholder="Replace"
            value={text.replace}
            onChange={(e) => setText(ws, (t) => ({ ...t, replace: e.target.value }))}
          />
        </div>
        <div className="sr-row2">
          <div className="sr-field">
            <input
              spellCheck={false}
              placeholder="Include"
              value={q.include}
              onChange={(e) => update((x) => ({ ...x, include: e.target.value }))}
            />
          </div>
          <div className="sr-field">
            <input
              spellCheck={false}
              placeholder="Exclude"
              value={q.exclude}
              onChange={(e) => update((x) => ({ ...x, exclude: e.target.value }))}
            />
          </div>
        </div>
        <div className="sr-chips">
          {SCOPES.map(([s, label]) => (
            <button
              key={s}
              type="button"
              className={`chip${scope === s ? " on" : ""}`}
              onClick={() => setText(ws, (t) => ({ ...t, scope: s }))}
            >
              {label}
            </button>
          ))}
        </div>
      </div>
      {q.pattern && (
        <div className="sr-sum">
          <b>{result.hits.length}</b>
          {result.truncated ? "+" : ""} results in <b>{groups.length}</b> files
          <span className="spacer" />
          <button
            type="button"
            className="sr-link"
            onClick={() => setFolded(folded.size ? new Set() : new Set(groups.map(([p]) => p)))}
          >
            {folded.size ? "Expand all" : "Collapse all"}
          </button>
        </div>
      )}
      {result.error && <div className="sr-error">{result.error}</div>}
      <div className="sr-res" ref={keys.listRef}>
        {groups.map(([path, list]) => {
          const shut = folded.has(path);
          return (
            <div key={path}>
              <button
                type="button"
                className="sr-file"
                onClick={() => {
                  const next = new Set(folded);
                  if (shut) next.delete(path);
                  else next.add(path);
                  setFolded(next);
                }}
              >
                <span className="sr-car">{shut ? "▸" : "▾"}</span>
                <FileName path={path} />
                <span className="sr-cnt">{list.length}</span>
              </button>
              {!shut &&
                list.map((h) => (
                  <HitLine
                    key={h.line}
                    hit={h}
                    on={shown[keys.at] === h}
                    changed={changed.has(`${h.path}:${h.line}`)}
                    onOpen={() => {
                      keys.setAt(shown.indexOf(h));
                      open(h);
                    }}
                  />
                ))}
            </div>
          );
        })}
      </div>
      <div className="sr-foot">
        <span>
          <kbd>↵</kbd> open
        </span>
        <span>
          <kbd>⌘↵</kbd> replace all
        </span>
        <span>
          <kbd>⌥↵</kbd> send to terminal
        </span>
      </div>
    </div>
  );
}

function Toggle({ on, label, onClick }: { on: boolean; label: string; onClick(): void }) {
  return (
    <button
      type="button"
      className={`sr-tog${on ? " on" : ""}`}
      aria-pressed={on}
      onClick={onClick}
    >
      {label}
    </button>
  );
}
