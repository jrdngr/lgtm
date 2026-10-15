import type { ReactNode } from "react";
import type { BotRun, Finding } from "../bindings";
import {
  finalFindings,
  locationText,
  type Report,
  raisers,
  refLabel,
  SEVERITIES,
} from "./findings";
import { Inline } from "./Markdown";
import { Progress } from "./Progress";
import { SevTag, Tally } from "./Severity";

/** Finding cards with their sources, settled ones folded at the end. */
type Cards = (list: [Finding, string][]) => ReactNode;

interface Props {
  run: BotRun;
  r: Report;
  /** Shows only the findings this persona raised. */
  persona: string | undefined;
  onPersona(name: string | undefined): void;
  cards: Cards;
  /** Whether a finding's card is open. */
  isOpen(id: string): boolean;
  onOpen(id: string): void;
  isChecked(id: string): boolean;
  onCheck(f: Finding, source: string): void;
}

const MINOR = new Set(["low", "info"]);

/** Every finding, worst first: cards for medium and up, a line each for low and info. A chip
 * per persona filters to the findings it raised. */
export function FindingsView(p: Props) {
  const { run, r, persona } = p;
  const source = (f: Finding) => (r.synthesis ? "synthesis" : (raisers(f)[0] ?? ""));
  const all = [...finalFindings(r)].sort(
    (a, b) => SEVERITIES.indexOf(a.severity) - SEVERITIES.indexOf(b.severity),
  );
  const shown = persona ? all.filter((f) => raisers(f).includes(persona)) : all;
  const major = shown.filter((f) => !MINOR.has(f.severity));
  const minor = shown.filter((f) => MINOR.has(f.severity));
  return (
    <>
      <div className="bots-filters">
        <button
          type="button"
          className="bots-chip"
          aria-pressed={!persona}
          onClick={() => p.onPersona(undefined)}
        >
          All
        </button>
        <Progress run={run} persona={persona} onPersona={p.onPersona} />
        <span className="spacer" />
        <Tally findings={shown} />
      </div>
      {p.cards(major.map((f) => [f, source(f)]))}
      {minor.length > 0 && (
        <section>
          <h3>Low and info</h3>
          {minor.map((f) => {
            const id = f.id ?? "";
            if (p.isOpen(id)) return <div key={id}>{p.cards([[f, source(f)]])}</div>;
            return (
              <div key={id} className="findings-line" data-finding={id}>
                <input
                  type="checkbox"
                  checked={p.isChecked(id)}
                  onChange={() => p.onCheck(f, source(f))}
                />
                <span className="findings-num">{refLabel(run, id)}</span>
                <SevTag severity={f.severity} />
                <button type="button" className="findings-title" onClick={() => p.onOpen(id)}>
                  <Inline>{f.title}</Inline>
                </button>
                <span className="suggested-loc">{f.locations.map(locationText).join(", ")}</span>
              </div>
            );
          })}
        </section>
      )}
    </>
  );
}
