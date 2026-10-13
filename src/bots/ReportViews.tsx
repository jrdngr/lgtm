import type { ReactNode } from "react";
import type { BotRun, Finding } from "../bindings";
import { Closing, Refs } from "./Closing";
import { findingsById, type Picked, type Report, SEVERITIES } from "./findings";
import { Markdown } from "./Markdown";
import { Progress } from "./Progress";
import { Tally } from "./Severity";

/** Finding cards with their sources, settled ones folded at the end. */
type Cards = (list: [Finding, string][]) => ReactNode;

function SelectAll({ onClick }: { onClick(): void }) {
  return (
    <button type="button" className="quiet" onClick={onClick} title="Pick every finding shown">
      Pick all
    </button>
  );
}

/** A verdict as a quote, with the tally of its findings and Pick all beside it. */
function Verdict({
  children,
  findings,
  onPickAll,
}: {
  children: ReactNode;
  findings: Finding[];
  onPickAll(): void;
}) {
  return (
    <div className="bots-verdict">
      <div className="bots-verdict-text">{children}</div>
      <div className="bots-verdict-side">
        <Tally findings={findings} />
        {findings.length > 0 && <SelectAll onClick={onPickAll} />}
      </div>
    </div>
  );
}

interface MinimalProps {
  r: Report;
  cards: Cards;
  checkAll(picks: Picked[]): void;
  sourceOf(f: Finding): string;
  /** Shows an off-task finding in the view that has it. */
  onGo(id: string): void;
}

/** The findings that matter for the task, with off-task critical ones linked. */
export function MinimalView({ r, cards, checkAll, sourceOf, onGo }: MinimalProps) {
  const m = r.minimal;
  if (!m) return null;
  const byId = findingsById(r);
  const kept = m.kept.flatMap((id) => byId.get(id) ?? []);
  const offTask = m.offTask.flatMap((id) => byId.get(id) ?? []);
  return (
    <>
      <Verdict
        findings={kept}
        onPickAll={() => checkAll(kept.map((f) => ({ f, source: sourceOf(f) })))}
      >
        For “{m.task}”: {kept.length} of {r.synthesis?.findings.length ?? byId.size} findings
      </Verdict>
      {cards(kept.map((f) => [f, sourceOf(f)]))}
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

interface SynthesisProps {
  r: Report;
  cards: Cards;
  checkAll(picks: Picked[]): void;
  onGo(id: string): void;
}

/** The synthesis's verdict and merged findings, then what to fix first and its questions. */
export function SynthesisView({ r, cards, checkAll, onGo }: SynthesisProps) {
  const s = r.synthesis;
  if (!s) return null;
  return (
    <>
      <Verdict
        findings={s.findings}
        onPickAll={() => checkAll(s.findings.map((f) => ({ f, source: "synthesis" })))}
      >
        <Markdown>{s.verdict}</Markdown>
      </Verdict>
      {cards(s.findings.map((f) => [f, "synthesis"]))}
      <Closing r={r} onGo={onGo} />
    </>
  );
}

interface PersonaProps {
  run: BotRun;
  r: Report;
  /** Shows only this persona's findings. */
  persona: string | undefined;
  onPersona(name: string | undefined): void;
  cards: Cards;
  checkAll(picks: Picked[]): void;
}

/** Every persona's findings, worst first, or one persona's with its verdict. */
export function PersonaView({ run, r, persona, onPersona, cards, checkAll }: PersonaProps) {
  const who = [...r.byPersona.keys()].filter((p) => !persona || p === persona);
  const findings = who.flatMap((p) => (r.byPersona.get(p) ?? []).map((f) => ({ p, f })));
  findings.sort((a, b) => SEVERITIES.indexOf(a.f.severity) - SEVERITIES.indexOf(b.f.severity));
  const verdict = persona && r.verdicts.get(persona);
  return (
    <>
      <div className="bots-filters">
        <Progress run={run} persona={persona} onPersona={onPersona} />
        <span className="spacer" />
        <Tally findings={findings.map((x) => x.f)} />
        {findings.length > 0 && (
          <SelectAll onClick={() => checkAll(findings.map(({ p, f }) => ({ f, source: p })))} />
        )}
      </div>
      {verdict && (
        <div className="bots-verdict">
          <div className="bots-verdict-text">
            <Markdown>{verdict}</Markdown>
          </div>
        </div>
      )}
      {cards(findings.map(({ p, f }) => [f, p]))}
    </>
  );
}
