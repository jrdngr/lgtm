import { useEffect, useMemo, useRef, useState } from "react";
import type { SysSymbolDto, SystemDto } from "../bindings";
import { useWorkspace } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import { Search } from "./glyphs";
import { nodeInfo } from "./names";
import { componentId } from "./scene";

interface Props {
  map: SystemDto;
  colors: Map<string, string>;
  onShow(id: string): void;
  onFile(path: string, line?: number): void;
  onClose(): void;
}

interface Hit {
  key: string;
  group: "Components" | "Files" | "Symbols";
  label: string;
  mono: boolean;
  component: string;
  path?: string;
  line?: number;
}

const COMPONENTS = 6;
const FILES = 8;
/** Waits this long after typing before asking for symbols. */
const DEBOUNCE = 150;

const base = (p: string) => p.slice(p.lastIndexOf("/") + 1);

function Marked({ text, q }: { text: string; q: string }) {
  const i = text.toLowerCase().indexOf(q.toLowerCase());
  if (!q || i < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, i)}
      <mark>{text.slice(i, i + q.length)}</mark>
      {text.slice(i + q.length)}
    </>
  );
}

/** Finds components, files and symbols by name. Enter shows one on the map; ⌘Enter opens its
 * file. */
export function FindPalette({ map, colors, onShow, onFile, onClose }: Props) {
  const workspace = useWorkspace();
  const [q, setQ] = useState("");
  const [at, setAt] = useState(0);
  const [symbols, setSymbols] = useState<{ q: string; found: SysSymbolDto[] }>();
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => input.current?.focus(), []);

  useEffect(() => {
    const query = q.trim();
    if (query.length < 2) return;
    let live = true;
    const t = setTimeout(() => {
      unwrap(commands.systemFind(workspace, query)).then(
        (found) => live && setSymbols({ q: query, found }),
        () => live && setSymbols({ q: query, found: [] }),
      );
    }, DEBOUNCE);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [workspace, q]);

  const hits = useMemo((): Hit[] => {
    const query = q.trim().toLowerCase();
    if (!query) return [];
    const comps = map.components
      .filter((k) => k.name.toLowerCase().includes(query))
      .sort((a, b) => a.name.length - b.name.length)
      .slice(0, COMPONENTS)
      .map(
        (k): Hit => ({
          key: `k:${k.id}`,
          group: "Components",
          label: k.name,
          mono: false,
          component: k.id,
        }),
      );
    const files = map.files
      .filter((f) => f.path.toLowerCase().includes(query))
      .sort(
        (a, b) =>
          Number(!base(b.path).toLowerCase().includes(query)) -
            Number(!base(a.path).toLowerCase().includes(query)) || a.path.length - b.path.length,
      )
      .slice(0, FILES)
      .map(
        (f): Hit => ({
          key: `f:${f.path}`,
          group: "Files",
          label: base(f.path),
          mono: true,
          component: f.component,
          path: f.path,
        }),
      );
    const syms =
      symbols && symbols.q.toLowerCase() === query
        ? symbols.found.map(
            (s): Hit => ({
              key: `s:${s.path}:${s.name}:${s.line}`,
              group: "Symbols",
              label: s.name,
              mono: true,
              component: s.component,
              path: s.path,
              line: s.line,
            }),
          )
        : [];
    return [...comps, ...files, ...syms];
  }, [map, q, symbols]);

  const pick = (h: Hit | undefined, open: boolean) => {
    if (!h) return;
    if (open && h.path) onFile(h.path, h.line);
    else onShow(componentId(h.component));
    onClose();
  };

  const onKey = (e: React.KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Escape") onClose();
    else if (e.key === "ArrowDown") setAt((i) => Math.min(hits.length - 1, i + 1));
    else if (e.key === "ArrowUp") setAt((i) => Math.max(0, i - 1));
    else if (e.key === "Enter") pick(hits[at], e.metaKey);
    else return;
    e.preventDefault();
  };

  const where = (h: Hit) => {
    const k = nodeInfo(map, colors, componentId(h.component));
    const c = k?.container !== undefined ? nodeInfo(map, colors, `c:${k.container}`) : undefined;
    if (!k) return null;
    return (
      <span className="sys-find-path">
        <span className="sys-sw" style={{ background: k.color }} />
        {h.group === "Components" ? c?.name : `${c?.name ?? ""} › ${k.name}`}
      </span>
    );
  };

  let group: string | undefined;
  return (
    <>
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: Escape closes it from the input */}
      <div className="sys-scrim" onClick={onClose} />
      <div className="sys-find" role="dialog" aria-label="Find">
        <label className="sys-find-q">
          <Search size={16} />
          <input
            ref={input}
            value={q}
            aria-label="Find"
            spellCheck={false}
            onChange={(e) => {
              setQ(e.target.value);
              setAt(0);
            }}
            onKeyDown={onKey}
          />
        </label>
        {!!hits.length && (
          <div className="sys-find-list">
            {hits.map((h, i) => {
              const head = h.group !== group;
              group = h.group;
              return (
                <div key={h.key}>
                  {head && <div className="sys-find-grp">{h.group}</div>}
                  <button
                    type="button"
                    className={`sys-find-r${i === at ? " on" : ""}`}
                    onMouseEnter={() => setAt(i)}
                    onClick={(e) => pick(h, e.metaKey)}
                  >
                    {h.group === "Components" && (
                      <span
                        className="sys-sw"
                        style={{
                          background: nodeInfo(map, colors, componentId(h.component))?.color,
                        }}
                      />
                    )}
                    <span className={h.mono ? "mono" : undefined}>
                      <Marked text={h.label} q={q.trim()} />
                    </span>
                    {where(h)}
                  </button>
                </div>
              );
            })}
          </div>
        )}
        <div className="sys-find-foot">
          <span>
            <kbd>↵</kbd> Show
          </span>
          <span>
            <kbd>⌘↵</kbd> Open File
          </span>
          <span>
            <kbd>esc</kbd> Close
          </span>
        </div>
      </div>
    </>
  );
}
