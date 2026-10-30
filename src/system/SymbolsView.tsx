import { type ReactNode, useEffect, useMemo, useState } from "react";
import type { SymbolsDto, SymNodeDto, SystemDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { commands, errorText, unwrap } from "../ipc";
import type { FlatFile } from "../review/model";
import { SymbolDetail } from "./SymbolDetail";
import { SymbolsCanvas } from "./SymbolsCanvas";
import { symbolsLayout, symKey } from "./symbolsLayout";

export type SymStatus = "added" | "changed";

interface Props {
  workspace: WorkspaceName;
  map: SystemDto;
  colors: Map<string, string>;
  component: string;
  /** The review's files, with the Review lens on. */
  files?: FlatFile[];
  /** Fades what the review doesn't change or reach. */
  changedOnly: boolean;
  /** A tour step's file or symbol. */
  tour?: { file?: string; symbol?: { path: string; name: string } };
  /** Replaces the detail, like a tour's steps. */
  aside?: ReactNode;
  /** The component's own detail, with no symbol selected. */
  fallback: ReactNode;
  /** Drawn over the canvas. */
  children?: ReactNode;
  /** The selected symbol or caller or call chip, by key. */
  selected?: string;
  onSelect(key: string | undefined): void;
  onShow(id: string): void;
  onFile(path: string, line?: number): void;
  onAsk(text: string): void;
}

/** The component's symbols, by file, with the symbols elsewhere that call them and that they
 * call. */
export function SymbolsView(p: Props) {
  const [got, setGot] = useState<{ component: string; dto?: SymbolsDto; error?: string }>();
  // biome-ignore lint/correctness/useExhaustiveDependencies: a reloaded map asks again
  useEffect(() => {
    let live = true;
    const component = p.component;
    unwrap(commands.systemSymbols(p.workspace, component)).then(
      (dto) => live && setGot({ component, dto }),
      (e) => live && setGot({ component, error: errorText(e) }),
    );
    return () => {
      live = false;
    };
  }, [p.workspace, p.component, p.map]);
  const mine = got?.component === p.component ? got : undefined;
  if (mine?.dto?.symbols.length) return <Loaded {...p} dto={mine.dto} />;
  return (
    <>
      <div className="sys-canvas">
        <div className={`placeholder${mine?.error ? " error" : ""}`}>
          {mine?.error ?? (mine?.dto ? "No symbols" : "Finding symbols…")}
        </div>
        {p.children}
      </div>
      {p.aside ?? p.fallback}
    </>
  );
}

/** Whether the review adds the symbol's file or changes rows in it. */
function symbolStatus(files: FlatFile[], s: SymNodeDto): SymStatus | undefined {
  const f = files.find((x) => x.dto.path === s.path);
  if (!f) return undefined;
  if (f.dto.status === "added") return "added";
  const hit = f.rows.some(
    (r) => r.kind === "added" && r.newLine !== null && r.newLine >= s.line && r.newLine <= s.end,
  );
  return hit ? "changed" : undefined;
}

function Loaded({ dto, ...p }: Props & { dto: SymbolsDto }) {
  const placed = useMemo(() => symbolsLayout(dto), [dto]);
  const status = useMemo(() => {
    const out = new Map<number, SymStatus>();
    if (!p.files) return out;
    for (const s of [...dto.symbols, ...dto.callers, ...dto.calls]) {
      const st = symbolStatus(p.files, s);
      if (st) out.set(s.id, st);
    }
    return out;
  }, [dto, p.files]);
  const tourSym = p.tour?.symbol;
  const tourId = tourSym
    ? dto.symbols.find((s) => s.path === tourSym.path && s.name === tourSym.name)?.id
    : undefined;
  const focus = tourId !== undefined ? symKey(tourId) : p.selected;
  // A reload can take the selection away.
  const known =
    focus !== undefined &&
    (placed.syms.some((s) => s.key === focus) || placed.chips.some((c) => c.key === focus));
  const sel = [...dto.symbols, ...dto.callers, ...dto.calls].find((s) => symKey(s.id) === focus);

  return (
    <>
      <SymbolsCanvas
        dto={dto}
        placed={placed}
        map={p.map}
        colors={p.colors}
        component={p.component}
        status={status}
        changedOnly={p.changedOnly}
        tourFile={p.tour?.file}
        focus={known ? focus : undefined}
        touring={tourId !== undefined}
        onSelect={p.onSelect}
        onFile={p.onFile}
      >
        {p.children}
      </SymbolsCanvas>
      {p.aside ??
        (sel ? (
          <SymbolDetail
            map={p.map}
            colors={p.colors}
            dto={dto}
            sym={sel}
            status={status.get(sel.id)}
            statusOf={(id) => status.get(id)}
            onSelect={(id) => p.onSelect(symKey(id))}
            onShow={p.onShow}
            onFile={p.onFile}
            onAsk={p.onAsk}
          />
        ) : (
          p.fallback
        ))}
    </>
  );
}
