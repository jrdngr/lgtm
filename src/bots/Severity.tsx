import type { Finding, Severity } from "../bindings";
import { SEVERITY_NAMES, tally } from "./findings";

/** A flat severity tag, colored by severity, with an optional count before it. */
export function SevTag({ severity, count }: { severity: Severity; count?: number }) {
  return (
    <span className={`sev sev-${severity}`}>
      {count !== undefined && `${count} `}
      {SEVERITY_NAMES[severity]}
    </span>
  );
}

/** How many findings there are of each severity, most severe first. */
export function Tally({ findings }: { findings: Finding[] }) {
  if (findings.length === 0) return <span className="bots-tally bots-none">No findings</span>;
  return (
    <span className="bots-tally">
      {tally(findings).map(([s, n]) => (
        <SevTag key={s} severity={s} count={n} />
      ))}
    </span>
  );
}
