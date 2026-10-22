import { EditorView } from "@codemirror/view";
import { useEffect, useMemo, useRef, useState } from "react";
import type { SymbolHit, SymbolKind } from "../bindings";
import { openFile } from "../editor/open";
import { commands, unwrap } from "../ipc";
import { Marked } from "./HitLine";
import { closeQuickPick, fuzzy, type Pick, parseQuery, useQuickPick } from "./pickStore";
import "./QuickPick.css";

const LIMIT = 50;

const KIND: Record<SymbolKind, { mark: string; cls: string }> = {
  function: { mark: "ƒ", cls: "fn" },
  class: { mark: "T", cls: "ty" },
  module: { mark: "M", cls: "mod" },
  section: { mark: "§", cls: "sec" },
};

interface Row {
  key: string;
  mark: string;
  cls: string;
  name: string;
  at: number[];
  where: string;
  go(): void;
}

/** Character indexes as [start, end) runs. */
function runs(at: number[]): [number, number][] {
  const out: [number, number][] = [];
  for (const i of at) {
    const last = out[out.length - 1];
    if (last && last[1] === i) last[1] = i + 1;
    else out.push([i, i + 1]);
  }
  return out;
}

/** The ⌘P picker: files, `@` symbols, and `:N` lines. */
export function QuickPickHost() {
  const pick = useQuickPick();
  return pick ? <QuickPick key={pick.id} pick={pick} /> : null;
}

function QuickPick({ pick }: { pick: Pick }) {
  const { ws, view } = pick;
  const [text, setText] = useState("");
  const [on, setOn] = useState(0);
  const [files, setFiles] = useState<string[]>([]);
  const [symbols, setSymbols] = useState<SymbolHit[]>([]);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    unwrap(commands.workspaceFiles(ws)).then(setFiles, () => setFiles([]));
  }, [ws]);

  const q = parseQuery(text);
  const symbolText = q.kind === "symbols" ? q.text : "";
  useEffect(() => {
    if (!symbolText) return setSymbols([]);
    let live = true;
    const t = setTimeout(() => {
      unwrap(commands.workspaceSymbols(ws, symbolText)).then(
        (s) => live && setSymbols(s),
        () => live && setSymbols([]),
      );
    }, 150);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [ws, symbolText]);

  function cancel() {
    closeQuickPick();
    if (pick.from instanceof HTMLElement) pick.from.focus();
  }

  const rows: Row[] = useMemo(() => {
    const q = parseQuery(text);
    if (q.kind === "line") {
      if (!view) return [];
      const n = Math.min(Math.max(q.line, 1), view.state.doc.lines);
      return [
        {
          key: "line",
          mark: ":",
          cls: "line",
          name: `Line ${n}`,
          at: [],
          where: "",
          go: () => {
            const pos = view.state.doc.line(n).from;
            view.dispatch({
              selection: { anchor: pos },
              effects: EditorView.scrollIntoView(pos, { y: "center" }),
            });
            view.focus();
          },
        },
      ];
    }
    if (q.kind === "symbols")
      return symbols.map((s) => ({
        key: `${s.path}:${s.line}:${s.name}`,
        ...KIND[s.kind],
        name: s.name,
        at: fuzzy(s.name, q.text)?.at ?? [],
        where: `${s.path}:${s.line}`,
        go: () => openFile(ws, s.path, s.line),
      }));
    const line = q.line;
    const found = q.text
      ? files
          .map((path) => ({ path, m: fuzzy(path, q.text) }))
          .filter((f) => f.m)
          .sort((a, b) => (b.m?.score ?? 0) - (a.m?.score ?? 0))
          .slice(0, LIMIT)
      : files.slice(0, LIMIT).map((path) => ({ path, m: undefined }));
    return found.map(({ path, m }) => {
      const cut = path.lastIndexOf("/") + 1;
      const name = path.slice(cut);
      return {
        key: path,
        mark: "≡",
        cls: "file",
        name,
        at: (m?.at ?? []).filter((i) => i >= cut).map((i) => i - cut),
        where: line ? `${path.slice(0, cut)}:${line}` : path.slice(0, cut),
        go: () => openFile(ws, path, line),
      };
    });
  }, [text, files, symbols, view, ws]);

  const at = Math.min(on, Math.max(rows.length - 1, 0));
  useEffect(() => {
    listRef.current?.children[at]?.scrollIntoView({ block: "nearest" });
  }, [at]);

  function choose(row: Row | undefined) {
    if (!row) return;
    closeQuickPick();
    row.go();
  }

  function onKey(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      cancel();
    } else if (e.key === "ArrowDown" || (e.ctrlKey && e.key === "n")) {
      e.preventDefault();
      setOn(Math.min(at + 1, rows.length - 1));
    } else if (e.key === "ArrowUp" || (e.ctrlKey && e.key === "p")) {
      e.preventDefault();
      setOn(Math.max(at - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(rows[at]);
    }
  }

  return (
    <div className="qp-backdrop" onMouseDown={cancel}>
      <div className="qp" onMouseDown={(e) => e.stopPropagation()}>
        <div className="sr-field">
          <input
            // biome-ignore lint/a11y/noAutofocus: the picker is for typing into
            autoFocus
            spellCheck={false}
            value={text}
            onChange={(e) => {
              setText(e.target.value);
              setOn(0);
            }}
            onKeyDown={onKey}
          />
        </div>
        {rows.length > 0 && (
          <div className="qp-list" ref={listRef}>
            {rows.map((r, i) => (
              <button
                type="button"
                key={r.key}
                className={`qp-row${i === at ? " on" : ""}`}
                onMouseMove={() => i !== at && setOn(i)}
                onClick={() => choose(r)}
              >
                <span className={`qp-kind ${r.cls}`}>{r.mark}</span>
                <span className="qp-name">
                  <Marked text={r.name} spans={runs(r.at)} />
                </span>
                <span className="qp-where">{r.where}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
