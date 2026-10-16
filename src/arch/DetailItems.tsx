import { commands, unwrap } from "../ipc";
import { PipeButton } from "../ui/PipeButton";
import type { DetailProps } from "./ArchDetail";
import { Pill, Sec, Site, Top } from "./DetailParts";
import { intentText, STATE_LABEL } from "./sidebar/logic";

const STATE_TONE = { broken: "red", open: "amber", met: "green", tag: "gray" } as const;

export function DetailLayer(p: DetailProps & { n: number }) {
  const layer = p.arch.layers.find((l) => l.n === p.n);
  if (!layer) return null;
  return (
    <>
      <Top
        what={
          <>
            <span className="anum">{layer.n}</span> {layer.title}
          </>
        }
        mono={false}
        onClose={p.onClose}
      />
      {layer.summary && <p className="areason muted">{layer.summary}</p>}
      {layer.ranges.length > 0 && <Sec>Ranges</Sec>}
      <div className="asites">
        {layer.ranges.map((r) => (
          <Site
            key={`${r.path}:${r.start}`}
            loc={`${r.path}:${r.start}${r.end > r.start ? `–${r.end}` : ""}`}
            onClick={() => p.review.show(r.path, r.start)}
          />
        ))}
      </div>
    </>
  );
}

export function DetailLook(p: DetailProps & { lookKey: string }) {
  const look = p.arch.look.find((l) => l.key === p.lookKey);
  if (!look) return null;
  const name = (id: string) => p.arch.components.find((c) => c.id === id)?.name ?? id;
  const { path, line, edge, component } = look;
  return (
    <>
      <Top what={look.title} mono={false} onClose={p.onClose}>
        <span className={`asevp ${look.severity}`}>{look.severity}</span>
        <Pill tone="gray">{look.source}</Pill>
      </Top>
      {look.detail && <p className="areason">{look.detail}</p>}
      {edge && (
        <button
          type="button"
          className="aedge-row"
          onClick={() => p.onSelect({ kind: "edge", from: edge[0], to: edge[1] })}
        >
          {name(edge[0])} → {name(edge[1])}
        </button>
      )}
      {!edge && component && (
        <button
          type="button"
          className="aedge-row"
          onClick={() => p.onSelect({ kind: "component", id: component })}
        >
          {name(component)}
        </button>
      )}
      {path && (
        <div className="asites">
          <Site loc={line ? `${path}:${line}` : path} onClick={() => p.review.show(path, line)} />
        </div>
      )}
    </>
  );
}

export function DetailIntent(p: DetailProps & { id: number }) {
  const it = p.arch.intents.find((i) => i.id === p.id);
  if (!it) return null;
  const name = (id: string) => p.arch.components.find((c) => c.id === id)?.name ?? id;
  const [what] = intentText(it, name);
  const edge =
    it.kind === "wrong"
      ? p.arch.edges.find((e) => e.from === it.from && e.to === it.to)
      : undefined;
  const reason = it.kind === "wrong" ? it.reason : it.kind === "note" ? it.text : null;
  const kind = {
    wrong: "Wrong",
    move: `Belongs in ${it.kind === "move" ? name(it.to) : ""}`,
    risk: "Risk",
    trust: "Trust boundary",
    note: "Note",
  }[it.kind];
  const checked = it.state !== "tag";
  return (
    <>
      <Top what={what} onClose={p.onClose}>
        <Pill tone={it.kind === "wrong" ? "red" : it.kind === "move" ? "amber" : "gray"}>
          {kind}
        </Pill>
        <Pill tone={STATE_TONE[it.state]}>{STATE_LABEL[it.state]}</Pill>
        {edge && (
          <Pill tone="gray">
            {edge.imports.length === 1 ? "1 site" : `${edge.imports.length} sites`}
          </Pill>
        )}
      </Top>
      {reason && <Sec>{it.kind === "note" ? "Note" : "Reason"}</Sec>}
      {reason && <p className="areason">{reason}</p>}
      {edge && edge.imports.length > 0 && <Sec>Sites</Sec>}
      {edge && (
        <div className="asites">
          {edge.imports.map((i) => (
            <Site
              key={`${i.fromPath}:${i.line}:${i.toPath}`}
              loc={`${i.fromPath}:${i.line}`}
              code={i.toPath}
              onClick={() => p.review.show(i.fromPath, i.line)}
            />
          ))}
        </div>
      )}
      <div className="aacts">
        {checked && it.state !== "met" && (
          <button
            type="button"
            className="abtn primary"
            disabled={p.fixing}
            onClick={() => p.fix([it.id])}
          >
            Fix
          </button>
        )}
        {checked && (
          <PipeButton
            className="abtn"
            onClick={() => p.send(unwrap(commands.intentPrompt(p.review.id, [it.id])))}
          >
            Intent
          </PipeButton>
        )}
        <button type="button" className="abtn" onClick={() => p.unmark(it.id)}>
          Unmark
        </button>
      </div>
    </>
  );
}
