import type { SymbolsDto, SymNodeDto, SystemDto } from "../bindings";
import { PipeButton } from "../terminal/PipeButton";
import { NEUTRAL } from "./colors";
import { nodeInfo } from "./names";
import type { SymStatus } from "./SymbolsView";
import { componentId, containerId } from "./scene";

interface Props {
  map: SystemDto;
  colors: Map<string, string>;
  dto: SymbolsDto;
  sym: SymNodeDto;
  status?: SymStatus;
  statusOf(id: number): SymStatus | undefined;
  onSelect(id: number): void;
  onShow(id: string): void;
  onFile(path: string, line?: number): void;
  onAsk(text: string): void;
}

/** Past this many, a list says how many more. */
const LIST = 5;

const base = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** What Ask sends a terminal agent about a symbol. */
export const symbolPrompt = (s: SymNodeDto) =>
  `Explain \`${s.name}\` in ${s.path}:${s.line}: what it does, who calls it and what it relies on. Keep it short.`;

/** The selected symbol: where it is, its signature, and its callers and calls. */
export function SymbolDetail(p: Props) {
  const { map, colors, dto, sym } = p;
  const all = [...dto.symbols, ...dto.callers, ...dto.calls];
  const byId = new Map(all.map((s) => [s.id, s]));
  const comp = nodeInfo(map, colors, componentId(sym.component));
  const box =
    comp?.container !== undefined ? nodeInfo(map, colors, containerId(comp.container)) : undefined;
  const callers = dto.edges.filter((e) => e.to === sym.id).flatMap((e) => byId.get(e.from) ?? []);
  const calls = dto.edges.filter((e) => e.from === sym.id).flatMap((e) => byId.get(e.to) ?? []);

  const list = (title: string, items: SymNodeDto[], count: boolean) =>
    !!items.length && (
      <section className="sys-sec">
        <h4>
          <span>{title}</span>
          {count && <span>{items.length}</span>}
        </h4>
        {items.slice(0, LIST).map((s) => {
          const st = p.statusOf(s.id);
          return (
            <button
              key={s.id}
              type="button"
              className="sys-li mono"
              onClick={() => p.onSelect(s.id)}
            >
              <span>{s.name}</span>
              <span className={`sys-at${st === "added" ? " add" : ""}`}>
                {st === "added" ? "new" : `${base(s.path)}:${s.line}`}
              </span>
            </button>
          );
        })}
        {items.length > LIST && <span className="sys-li more">{items.length - LIST} more</span>}
      </section>
    );

  return (
    <aside className="sys-detail">
      <header className="sys-head">
        {comp && (
          <button
            type="button"
            className="sys-loc"
            onClick={() => p.onShow(componentId(sym.component))}
          >
            <span className="sys-sw" style={{ background: comp.color ?? NEUTRAL }} />
            {box ? `${box.name} › ${comp.name}` : comp.name}
          </button>
        )}
        <span className="sys-title mono">
          {sym.name}
          {p.status && (
            <span className={`sys-chip ${p.status === "added" ? "grn" : "amb"}`}>
              {p.status === "added" ? "New" : "Changed"}
            </span>
          )}
        </span>
        {sym.signature && <div className="sys-sig">{sym.signature}</div>}
        <div className="sys-acts">
          <button type="button" className="sys-btn" onClick={() => p.onFile(sym.path, sym.line)}>
            Open
            <span className="sys-btn-at">
              {base(sym.path)}:{sym.line}
            </span>
          </button>
          <PipeButton onClick={() => p.onAsk(symbolPrompt(sym))}>Ask</PipeButton>
        </div>
      </header>
      {list("Callers", callers, true)}
      {list("Calls", calls, false)}
    </aside>
  );
}
