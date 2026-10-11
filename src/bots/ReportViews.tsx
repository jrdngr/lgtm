import type { ReactNode } from "react";
import type { Finding } from "../bindings";
import { Refs, Tally } from "./Closing";
import { findingsById, type Picked, type Report, SEVERITIES } from "./findings";
import { Markdown } from "./Markdown";

type Card = (f: Finding, source: string) => ReactNode;

export function SelectAll({ onClick }: { onClick(): void }) {
  return (
    <button type="button" onClick={onClick} title="Pick every finding shown">
      Pick all
    </button>
  );
}

interface MinimalProps {
  r: Report;
  card: Card;
  checkAll(picks: Picked[]): void;
  sourceOf(f: Finding): string;
  /** Shows an off-task finding in the view that has it. */
  onGo(id: string): void;
}

/** The findings that matter for the task, with off-task critical ones linked. */
export function MinimalView({ r, card, checkAll, sourceOf, onGo }: MinimalProps) {
  const m = r.minimal;
  if (!m) return null;
  const byId = findingsById(r);
  const kept = m.kept.flatMap((id) => byId.get(id) ?? []);
  const offTask = m.offTask.flatMap((id) => byId.get(id) ?? []);
  return (
    <>
      <div className="bots-verdict">
        For “{m.task}” · {kept.length} of {r.synthesis?.findings.length ?? byId.size} findings{" "}
        <Tally findings={kept} />
        <SelectAll onClick={() => checkAll(kept.map((f) => ({ f, source: sourceOf(f) })))} />
      </div>
      {kept.map((f) => card(f, sourceOf(f)))}
      {offTask.length > 0 && (
        <section>
          <h3>Off-task critical and high</h3>
          <Refs ids={m.offTask} onGo={onGo} />
        </section>
      )}
      <section>
        <h3>Recommendations</h3>
        <Markdown>{m.recommendations}</Markdown>
      </section>
    </>
  );
}

interface PersonaProps {
  r: Report;
  /** Shows only this persona's findings. */
  persona: string | undefined;
  card: Card;
  checkAll(picks: Picked[]): void;
}

/** Every persona's findings, worst first, or one persona's with its verdict. */
export function PersonaView({ r, persona, card, checkAll }: PersonaProps) {
  const who = [...r.byPersona.keys()].filter((p) => !persona || p === persona);
  const findings = who.flatMap((p) => (r.byPersona.get(p) ?? []).map((f) => ({ p, f })));
  findings.sort((a, b) => SEVERITIES.indexOf(a.f.severity) - SEVERITIES.indexOf(b.f.severity));
  const verdict = persona && r.verdicts.get(persona);
  return (
    <>
      {verdict && (
        <div className="bots-verdict">
          <Markdown>{verdict}</Markdown>
        </div>
      )}
      <div className="bots-verdict">
        <Tally findings={findings.map((x) => x.f)} />
        <SelectAll onClick={() => checkAll(findings.map(({ p, f }) => ({ f, source: p })))} />
      </div>
      {findings.map(({ p, f }) => card(f, p))}
    </>
  );
}
