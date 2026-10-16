import type { ArchDto, ComponentDto, EdgeDto, FileRefDto } from "../bindings";
import type { CurrentReview } from "../review/current";
import { startItemDrag } from "./ArchNode";
import { edgeText } from "./actions";
import { DetailIntent, DetailLayer, DetailLook } from "./DetailItems";
import { Pill, Sec, Site, Top } from "./DetailParts";
import { edgeKey } from "./scene";
import { intentText, STATE_LABEL } from "./sidebar/logic";
import type { ArchSelection } from "./state";

export interface DetailProps {
  review: CurrentReview;
  arch: ArchDto;
  selection: ArchSelection;
  onSelect(sel: ArchSelection): void;
  onClose(): void;
  /** Pastes the text into a terminal. */
  send(text: Promise<string>): void;
  markWrong(e: EdgeDto): void;
  unmark(id: number): void;
  fix(ids: number[]): void;
  fixing: boolean;
}

/** The selected component, edge, layer, Look item or intent, beside the graph. */
export function ArchDetail(p: DetailProps) {
  const sel = p.selection;
  const body = (() => {
    switch (sel.kind) {
      case "component": {
        const c = p.arch.components.find((x) => x.id === sel.id);
        return c && <DetailComponent {...p} c={c} />;
      }
      case "edge": {
        const e = p.arch.edges.find((x) => x.from === sel.from && x.to === sel.to);
        return e && <DetailEdge {...p} e={e} />;
      }
      case "layer":
        return <DetailLayer {...p} n={sel.n} />;
      case "look":
        return <DetailLook {...p} lookKey={sel.key} />;
      case "intent":
        return <DetailIntent {...p} id={sel.id} />;
    }
  })();
  if (!body) return null;
  return <div className="adetail">{body}</div>;
}

const STATUS_PILL: Partial<Record<ComponentDto["status"], [string, string]>> = {
  added: ["green", "New"],
  removed: ["gray", "Removed"],
  changed: ["amber", "Changed"],
  upstream: ["gray", "Upstream"],
  impacted: ["violet", "Impacted"],
};

const FILE_SIGN = { added: "+", removed: "−" } as Record<string, string>;

function FileRow({
  review,
  f,
  misplaced,
}: {
  review: CurrentReview;
  f: FileRefDto;
  misplaced: boolean;
}) {
  const status = f.file === null ? undefined : review.files[f.file]?.dto.status;
  const sign = status ? (FILE_SIGN[status] ?? "~") : "";
  return (
    <button
      type="button"
      className={`afile${misplaced ? " misplaced" : ""}`}
      draggable
      onDragStart={(e) => startItemDrag(e, { path: f.path, symbol: null })}
      onClick={() => review.show(f.path, null)}
    >
      <span className={`apre ${sign === "+" ? "add" : sign === "−" ? "rem" : "mod"}`}>{sign}</span>
      <span className="apath">{f.path}</span>
    </button>
  );
}

function EdgeRow({
  arch,
  e,
  out,
  onSelect,
}: {
  arch: ArchDto;
  e: EdgeDto;
  out: boolean;
  onSelect(sel: ArchSelection): void;
}) {
  const other = arch.components.find((c) => c.id === (out ? e.to : e.from))?.name;
  const tone =
    e.wrong !== null && e.status !== "removed"
      ? "wrong"
      : e.status === "added"
        ? "add"
        : e.status === "removed"
          ? "rem"
          : e.impacted
            ? "imp"
            : "";
  return (
    <button
      type="button"
      className={`aedge-row ${tone}`}
      onClick={() => onSelect({ kind: "edge", from: e.from, to: e.to })}
    >
      {out ? `→ ${other}` : `← ${other}`}
    </button>
  );
}

function DetailComponent(p: DetailProps & { c: ComponentDto }) {
  const { c, arch } = p;
  const pill = STATUS_PILL[c.status];
  const out = arch.edges.filter((e) => e.from === c.id);
  const into = arch.edges.filter((e) => e.to === c.id);
  const intents = arch.intents.filter(
    (i) =>
      (i.kind === "wrong" && (i.from === c.id || i.to === c.id)) ||
      (i.kind === "move" && i.to === c.id) ||
      ((i.kind === "risk" || i.kind === "trust" || i.kind === "note") && i.component === c.id),
  );
  const looks = arch.look.filter((l) => l.component === c.id);
  const misplaced = new Set(c.misplaced.map((m) => m.split("#")[0]));
  const name = (id: string) => arch.components.find((x) => x.id === id)?.name ?? id;
  return (
    <>
      <Top what={c.name} onClose={p.onClose}>
        {c.layer !== null && <span className="anum">{c.layer}</span>}
        {pill && <Pill tone={pill[0]}>{pill[1]}</Pill>}
        {c.risk && <Pill tone="red">Risk</Pill>}
        {c.trust && <Pill tone="red">Trust boundary</Pill>}
        {c.misplaced.length > 0 && <Pill tone="amber">{c.misplaced.length} misplaced</Pill>}
      </Top>
      {c.note && <p className="areason">{c.note}</p>}
      <Sec>Files</Sec>
      <div className="alist">
        {c.files.map((f) => (
          <FileRow key={f.path} review={p.review} f={f} misplaced={misplaced.has(f.path)} />
        ))}
      </div>
      {out.length > 0 && <Sec>Uses</Sec>}
      {out.length > 0 && (
        <div className="alist">
          {out.map((e) => (
            <EdgeRow key={edgeKey(e.from, e.to)} arch={arch} e={e} out onSelect={p.onSelect} />
          ))}
        </div>
      )}
      {into.length > 0 && <Sec>Used by</Sec>}
      {into.length > 0 && (
        <div className="alist">
          {into.map((e) => (
            <EdgeRow
              key={edgeKey(e.from, e.to)}
              arch={arch}
              e={e}
              out={false}
              onSelect={p.onSelect}
            />
          ))}
        </div>
      )}
      {intents.length > 0 && <Sec>Intent</Sec>}
      {intents.length > 0 && (
        <div className="alist">
          {intents.map((i) => (
            <button
              key={i.id}
              type="button"
              className="aitem"
              onClick={() => p.onSelect({ kind: "intent", id: i.id })}
            >
              <span className="awhat-sm">
                {intentText(i, name)[i.kind === "wrong" || i.kind === "move" ? 0 : 1]}
              </span>
              <span className={`ast ${i.state}`}>{STATE_LABEL[i.state]}</span>
            </button>
          ))}
        </div>
      )}
      {looks.length > 0 && <Sec>Look</Sec>}
      {looks.length > 0 && (
        <div className="alist">
          {looks.map((l) => (
            <button
              key={l.key}
              type="button"
              className={`aitem${l.checked ? " done" : ""}`}
              onClick={() => p.onSelect({ kind: "look", key: l.key })}
            >
              <span className="awhat-sm">{l.title}</span>
              <span className={`asevp ${l.severity}`}>{l.severity}</span>
            </button>
          ))}
        </div>
      )}
    </>
  );
}

function DetailEdge(p: DetailProps & { e: EdgeDto }) {
  const { e, arch } = p;
  const name = (id: string) => arch.components.find((c) => c.id === id)?.name ?? id;
  const intent = e.wrong === null ? undefined : arch.intents.find((i) => i.id === e.wrong);
  const reason = intent?.kind === "wrong" ? intent.reason : null;
  const sites = e.imports.length;
  return (
    <>
      <Top what={`${name(e.from)} → ${name(e.to)}`} onClose={p.onClose}>
        {intent && <Pill tone="red">Wrong</Pill>}
        {intent && intent.state !== "met" && (
          <Pill tone="red">{intent.state === "broken" ? "Broken" : "Open"}</Pill>
        )}
        {e.status === "added" && <Pill tone="green">New</Pill>}
        {e.status === "removed" && <Pill tone="gray">Removed</Pill>}
        {e.impacted && <Pill tone="violet">Impacted</Pill>}
        <Pill tone="gray">{sites === 1 ? "1 site" : `${sites} sites`}</Pill>
      </Top>
      {reason && <Sec>Reason</Sec>}
      {reason && <p className="areason">{reason}</p>}
      <Sec>Sites</Sec>
      <div className="asites">
        {e.imports.map((i) => (
          <Site
            key={`${i.fromPath}:${i.line}:${i.toPath}`}
            loc={`${i.fromPath}:${i.line}`}
            code={i.toPath}
            onClick={() => p.review.show(i.fromPath, i.line)}
          />
        ))}
      </div>
      <div className="aacts">
        {intent ? (
          <>
            <button
              type="button"
              className="abtn primary"
              disabled={p.fixing}
              onClick={() => p.fix([intent.id])}
            >
              Fix
            </button>
            <SendButton {...p} />
            <button type="button" className="abtn" onClick={() => p.unmark(intent.id)}>
              Unmark
            </button>
          </>
        ) : (
          <>
            <button type="button" className="abtn" onClick={() => p.markWrong(e)}>
              Mark Wrong…
            </button>
            <SendButton {...p} />
          </>
        )}
      </div>
    </>
  );
}

function SendButton(p: DetailProps & { e: EdgeDto }) {
  return (
    <button
      type="button"
      className="abtn"
      onClick={() => p.send(edgeText(p.review.id, p.arch, p.e))}
    >
      Send to terminal
    </button>
  );
}
