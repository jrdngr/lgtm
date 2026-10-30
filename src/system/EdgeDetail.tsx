import { useEffect, useState } from "react";
import type { EdgeSitesDto, LookDto, SystemDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import { PipeButton } from "../terminal/PipeButton";
import { LookList } from "./LensSections";
import type { Lens } from "./lens";
import { nodeInfo } from "./names";
import type { SEdge } from "./scene";

interface Props {
  workspace: WorkspaceName;
  map: SystemDto;
  colors: Map<string, string>;
  edge: SEdge;
  /** The ends picked, when the canvas draws them as `edge` into a door. */
  ends?: { from: string; to: string };
  lens?: Lens;
  onShow(id: string): void;
  onFile(path: string, line?: number): void;
  onTrace(): void;
  onAsk(text: string): void;
  onLook(l: LookDto): void;
}

const base = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** What Ask sends a terminal agent about an edge. */
const edgePrompt = (from: string, to: string) =>
  `Explain how ${from} uses ${to} in this repository: what it imports and why. Keep it short and point me at the import sites.`;

/** An import between two components or containers: where it happens and what it takes. */
export function EdgeDetail(p: Props) {
  const { map, colors, edge } = p;
  const ends = p.ends ?? edge;
  const exact = ends.from === edge.from && ends.to === edge.to;
  const from = nodeInfo(map, colors, ends.from);
  const to = nodeInfo(map, colors, ends.to);
  const components = ends.from.startsWith("k:") && ends.to.startsWith("k:");
  const [got, setGot] = useState<{ key: string; dto: EdgeSitesDto }>();
  const key = `${ends.from}>${ends.to}`;
  // biome-ignore lint/correctness/useExhaustiveDependencies: a reloaded map asks again
  useEffect(() => {
    if (!components) return;
    let live = true;
    unwrap(commands.systemEdge(p.workspace, ends.from.slice(2), ends.to.slice(2))).then(
      (dto) => live && setGot({ key, dto }),
      () => live && setGot({ key, dto: { sites: [], names: [] } }),
    );
    return () => {
      live = false;
    };
  }, [p.workspace, key, components, map]);
  const dto = got?.key === key ? got.dto : undefined;
  /** Whether component `k` is node `end` or lies in it. */
  const within = (end: string, k: string) =>
    end.startsWith("k:")
      ? end.slice(2) === k
      : map.components.find((c) => c.id === k)?.container === end.slice(2);
  const look = [...(p.lens?.edgeLook ?? [])].flatMap(([k, items]) => {
    const [a = "", b = ""] = k.split(">");
    return within(ends.from, a) && within(ends.to, b) ? items : [];
  });
  // Between containers, the component edges that make it up.
  const parts = components
    ? []
    : map.edges
        .filter((e) => within(ends.from, e.from) && within(ends.to, e.to))
        .sort((a, b) => b.imports - a.imports);
  const sites = exact
    ? edge.imports
    : map.edges.find((e) => `k:${e.from}` === ends.from && `k:${e.to}` === ends.to)?.imports;
  const status = exact ? edge.status : undefined;
  return (
    <aside className="sys-detail">
      <header className="sys-head">
        <span className="sys-title">
          {from?.name ?? ends.from}
          <span className="sys-to">→</span>
          {to?.name ?? ends.to}
          {status === "added" && <span className="sys-chip grn">New</span>}
          {status === "removed" && <span className="sys-chip red">Removed</span>}
          {!!sites && (
            <span className="sys-chip">
              {sites} {sites === 1 ? "site" : "sites"}
            </span>
          )}
        </span>
        {exact && edge.label && <p>{edge.label}</p>}
        <div className="sys-acts">
          <button type="button" className="sys-btn" onClick={p.onTrace}>
            Trace
          </button>
          <PipeButton onClick={() => p.onAsk(edgePrompt(from?.name ?? "", to?.name ?? ""))}>
            Ask
          </PipeButton>
        </div>
      </header>
      <LookList items={look} onLook={p.onLook} />
      {!!dto?.sites.length && (
        <section className="sys-sec">
          <h4>
            <span>Sites</span>
            <span>{dto.sites.length}</span>
          </h4>
          {dto.sites.map((s) => (
            <button
              key={`${s.path}:${s.line}`}
              type="button"
              className="sys-li mono"
              onClick={() => p.onFile(s.path, s.line)}
            >
              <span>{s.path.split("/").slice(-2).join("/")}</span>
              <span className="sys-at">:{s.line}</span>
            </button>
          ))}
        </section>
      )}
      {!!dto?.names.length && (
        <section className="sys-sec">
          <h4>Imports</h4>
          {dto.names.map((s) => (
            <button
              key={`${s.path}:${s.name}`}
              type="button"
              className="sys-li mono"
              onClick={() => p.onFile(s.path, s.line)}
            >
              <span>{s.name}</span>
              <span className="sys-at">{base(s.path)}</span>
            </button>
          ))}
        </section>
      )}
      {!!parts.length && (
        <section className="sys-sec">
          <h4>
            <span>Components</span>
            <span>{parts.length}</span>
          </h4>
          {parts.map((e) => {
            const a = map.components.find((k) => k.id === e.from);
            const b = map.components.find((k) => k.id === e.to);
            return (
              <button
                key={`${e.from}>${e.to}`}
                type="button"
                className="sys-li"
                onClick={() => p.onShow(`k:${e.from}>k:${e.to}`)}
              >
                <span>
                  {a?.name} → {b?.name}
                </span>
                <span className="sys-n">{e.imports}</span>
              </button>
            );
          })}
        </section>
      )}
    </aside>
  );
}
