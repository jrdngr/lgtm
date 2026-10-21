import type { DesignBuilt, DesignChange, DesignMapDto, SpecRefDto } from "../bindings";
import { PipeButton } from "../terminal/PipeButton";
import { nodeName } from "./designAsk";
import type { DesignNode } from "./designScene";
import { dcid, dkid, specOf } from "./designScene";
import "./design.css";

const CHIPS: Record<DesignChange | DesignBuilt, string | undefined> = {
  new: "New",
  changed: "Changes",
  removed: "Removed",
  same: undefined,
  built: "✓",
  planned: "Planned",
  differs: "Differs",
};

interface Props {
  m: DesignMapDto;
  /** The selection; the design as a whole when absent. */
  node?: DesignNode;
  /** Whether the code has been compared with the design. */
  drift: boolean;
  overCode: boolean;
  onShow(id: string): void;
  onFile(path: string, line?: number): void;
  onUpdateSpec(): void;
}

/** A design node's id on the map, from its id in the design. */
function mapId(m: DesignMapDto, end: string) {
  if (m.components.some((k) => k.id === end)) return dkid(end);
  if (m.containers.some((c) => c.id === end)) return dcid(end);
  return m.people.some((p) => p.name === end) ? `p:${end}` : `x:${end}`;
}

function endName(m: DesignMapDto, end: string) {
  return (
    m.components.find((k) => k.id === end)?.name ??
    m.containers.find((c) => c.id === end)?.name ??
    end
  );
}

function Quote({ r, onFile }: { r: SpecRefDto; onFile: Props["onFile"] }) {
  return (
    <div className="dsn-quote">
      <button type="button" className="dsn-link" onClick={() => onFile(r.path, r.line)}>
        {r.path.slice(r.path.lastIndexOf("/") + 1)}:{r.line}
      </button>
      <span>{r.text}</span>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="dsn-sec">
      <h4>{title}</h4>
      {children}
    </section>
  );
}

/** The node's state on the map: what the design changes, or once code lands, whether it's
 * built. */
function chipOf(n: DesignNode, drift: boolean, overCode: boolean) {
  if (!overCode) return undefined;
  const of = (change: DesignChange, built: DesignBuilt | null) =>
    drift && change !== "removed" && built ? built : change;
  if (n.kind === "container") return of(n.c.change, n.c.built);
  if (n.kind === "component") return of(n.k.change, n.k.built);
  if (n.kind === "interface") return of(n.k.change, n.k.interfaces[n.at]?.built ?? null);
  return undefined;
}

/** The selection's description, the spec lines behind it, what it talks to, and with code
 * compared, where the code differs. */
export function DesignDetail(p: Props) {
  const { m, node } = p;
  if (!node) return <Overview {...p} />;
  const { name, id } = nodeName(node);
  const chip = chipOf(node, p.drift, p.overCode);
  const spec = specOf(node);
  const differences = p.drift ? m.differences.filter((x) => x.node === id) : [];
  const iface = node.kind === "interface" ? node.k.interfaces[node.at] : undefined;
  const description =
    iface?.description ??
    (node.kind === "container"
      ? node.c.description
      : node.kind === "component"
        ? node.k.description
        : node.kind === "person" || node.kind === "external"
          ? node.description
          : "");
  const relations = m.relations.filter((r) => r.from === id || r.to === id);
  const replaces =
    node.kind === "container"
      ? m.containers.filter((c) => node.c.replaces.includes(c.id))
      : node.kind === "component"
        ? m.components.filter((k) => node.k.replaces.includes(k.id))
        : [];
  const inside =
    node.kind === "container" && p.drift
      ? m.components.filter((k) => k.container === node.c.id && k.change !== "removed")
      : [];
  return (
    <aside className="sys-detail dsn-detail">
      {iface && node.kind === "interface" ? (
        <div className="dsn-detail-iface">
          <button type="button" className="dsn-up" onClick={() => p.onShow(dkid(node.k.id))}>
            {node.k.name}
          </button>
          <b className="dsn-sig">{iface.signature || iface.name}</b>
        </div>
      ) : (
        <div className="dsn-detail-hd">
          {chip && chip !== "same" && <span className={`dsn-ring ${chip}`} />}
          <b>{name}</b>
          {chip && CHIPS[chip] && <span className={`dsn-chip ${chip}`}>{CHIPS[chip]}</span>}
        </div>
      )}
      {description && <div className="dsn-desc">{description}</div>}
      {!!differences.length && (
        <Section title="DIFFERS FROM SPEC">
          {differences.map((x, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: differences can repeat
            <div key={i} className="dsn-quote differs">
              <span>{x.text}</span>
              {x.path && (
                <button
                  type="button"
                  className="dsn-link"
                  onClick={() => x.path && p.onFile(x.path, x.line ?? undefined)}
                >
                  {x.path}
                  {x.line ? `:${x.line}` : ""}
                </button>
              )}
            </div>
          ))}
        </Section>
      )}
      {!differences.length && !!spec.length && (
        <Section title="SPEC">
          {spec.map((r) => (
            <Quote key={`${r.path}:${r.line}`} r={r} onFile={p.onFile} />
          ))}
        </Section>
      )}
      {node.kind === "interface" ? (
        <Calls m={m} id={id} onShow={p.onShow} />
      ) : (
        !!relations.length && (
          <Section title="TALKS TO">
            {relations.map((r) => {
              const other = r.from === id ? r.to : r.from;
              return (
                <button
                  key={`${r.from}>${r.to}`}
                  type="button"
                  className={`dsn-pair${r.change === "removed" ? " gone" : ""}`}
                  onClick={() => p.onShow(mapId(m, other))}
                >
                  <span>{endName(m, other)}</span>
                  <span>{r.label}</span>
                </button>
              );
            })}
          </Section>
        )
      )}
      {!!replaces.length && (
        <Section title="REPLACES">
          {replaces.map((r) => (
            <button
              key={r.id}
              type="button"
              className="dsn-pair gone"
              onClick={() => p.onShow(mapId(m, r.id))}
            >
              <span>{r.name}</span>
              <span>{"tech" in r ? r.tech : ""}</span>
            </button>
          ))}
        </Section>
      )}
      {!!inside.length && (
        <Section title="COMPONENTS">
          {inside.map((k) => (
            <button
              key={k.id}
              type="button"
              className="dsn-pair"
              onClick={() => p.onShow(dkid(k.id))}
            >
              <span>{k.name}</span>
              {k.built && <span className={`dsn-st ${k.built}`}>{CHIPS[k.built]}</span>}
            </button>
          ))}
        </Section>
      )}
      <span className="sys-grow" />
      <div className="dsn-acts">
        {!!differences.length && <PipeButton onClick={p.onUpdateSpec}>Update spec</PipeButton>}
        {!differences.length && spec[0] && (
          <button
            type="button"
            className="dsn-btn"
            onClick={() => spec[0] && p.onFile(spec[0].path, spec[0].line)}
          >
            Open spec
          </button>
        )}
      </div>
    </aside>
  );
}

/** What uses an interface's component and what it uses. */
function Calls({ m, id, onShow }: { m: DesignMapDto; id: string; onShow(id: string): void }) {
  const by = m.relations.filter((r) => r.to === id).map((r) => r.from);
  const calls = m.relations.filter((r) => r.from === id).map((r) => r.to);
  const list = (ends: string[]) =>
    ends.map((e) => (
      <button key={e} type="button" className="dsn-pair" onClick={() => onShow(mapId(m, e))}>
        <span>{endName(m, e)}</span>
      </button>
    ));
  return (
    <>
      {!!by.length && <Section title="CALLED BY">{list(by)}</Section>}
      {!!calls.length && <Section title="CALLS">{list(calls)}</Section>}
    </>
  );
}

/** The design as a whole: its summary and containers. */
function Overview(p: Props) {
  const { m } = p;
  return (
    <aside className="sys-detail dsn-detail">
      {m.summary && <div className="dsn-desc">{m.summary}</div>}
      <Section title="CONTAINERS">
        {m.containers.map((c) => {
          const chip = p.overCode
            ? p.drift && c.change !== "removed" && c.built
              ? c.built
              : c.change
            : undefined;
          return (
            <button
              key={c.id}
              type="button"
              className={`dsn-pair${c.change === "removed" ? " gone" : ""}`}
              onClick={() => p.onShow(dcid(c.id))}
            >
              <span>{c.name}</span>
              {chip && CHIPS[chip] && <span className={`dsn-st ${chip}`}>{CHIPS[chip]}</span>}
            </button>
          );
        })}
      </Section>
    </aside>
  );
}
