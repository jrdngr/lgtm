import type {
  BotRun,
  Challenge,
  Finding,
  Location,
  Minimal,
  Output,
  Response,
  Ruling,
  Severity,
  Synthesis,
} from "../bindings";
import type { FlatFile } from "../review/model";

export const SEVERITIES: Severity[] = ["critical", "high", "medium", "low", "info"];

export const BADGES: Record<Severity, string> = {
  critical: "🔴 CRITICAL",
  high: "🟠 HIGH",
  medium: "🟡 MEDIUM",
  low: "🔵 LOW",
  info: "⚪ INFO",
};

type Of<K extends Output["kind"]> = Extract<Output, { kind: K }>;

/** The done parts of a run's step, with their output and persona. */
function outputs<K extends Output["kind"]>(run: BotRun, kind: K) {
  return run.parts.flatMap((p) =>
    p.status === "done" && p.output?.kind === kind
      ? [{ persona: p.persona ?? "", output: p.output as Of<K> }]
      : [],
  );
}

/** A challenge or rebuttal on a persona finding, with who made it. */
export type Exchange =
  | ({ kind: "challenge"; by: string } & Challenge)
  | ({ kind: "response"; by: string } & Response);

/** Everything the report pane shows of a run, gathered from its parts. */
export interface Report {
  /** Each persona's findings, by persona name. */
  byPersona: Map<string, Finding[]>;
  verdicts: Map<string, string>;
  synthesis?: Synthesis;
  minimal?: Minimal;
  exchanges: Exchange[];
}

export function report(run: BotRun): Report {
  const byPersona = new Map<string, Finding[]>();
  const verdicts = new Map<string, string>();
  for (const { persona, output } of outputs(run, "review")) {
    byPersona.set(persona, output.findings);
    verdicts.set(persona, output.verdict);
  }
  const exchanges: Exchange[] = [
    ...outputs(run, "challenges").flatMap(({ persona, output }) =>
      output.challenges.map((c) => ({ kind: "challenge" as const, by: persona, ...c })),
    ),
    ...outputs(run, "rebuttals").flatMap(({ persona, output }) =>
      output.responses.map((r) => ({ kind: "response" as const, by: persona, ...r })),
    ),
  ];
  return {
    byPersona,
    verdicts,
    synthesis: outputs(run, "synthesis")[0]?.output,
    minimal: outputs(run, "minimal")[0]?.output,
    exchanges,
  };
}

/** The persona findings a finding stands for: its sources, or itself. */
export function sourceIds(f: Finding): string[] {
  return f.sources?.length ? f.sources : f.id ? [f.id] : [];
}

/** The cross-review exchanges and rulings behind a finding. */
export function history(r: Report, f: Finding): { exchanges: Exchange[]; rulings: Ruling[] } {
  const ids = new Set(sourceIds(f));
  return {
    exchanges: r.exchanges.filter((e) => ids.has(e.finding)),
    rulings: (r.synthesis?.rulings ?? []).filter((x) => ids.has(x.finding)),
  };
}

/** Findings by id, from the synthesis and every persona, for resolving references. */
export function findingsById(r: Report): Map<string, Finding> {
  const out = new Map<string, Finding>();
  for (const fs of r.byPersona.values()) for (const f of fs) if (f.id) out.set(f.id, f);
  for (const f of r.synthesis?.findings ?? []) if (f.id) out.set(f.id, f);
  return out;
}

export function tally(findings: Finding[]): [Severity, number][] {
  return SEVERITIES.map((s): [Severity, number] => [
    s,
    findings.filter((f) => f.severity === s).length,
  ]).filter(([, n]) => n > 0);
}

export function locationText(l: Location): string {
  if (l.start === null) return l.path;
  return l.end !== null && l.end !== l.start
    ? `${l.path}:${l.start}-${l.end}`
    : `${l.path}:${l.start}`;
}

/** A finding as markdown, for a comment or an agent's prompt. */
export function findingText(f: Finding, source: string): string {
  const parts = [
    `**${BADGES[f.severity]} — ${f.title}** (${source})`,
    `Where: ${f.locations.map((l) => `\`${locationText(l)}\``).join(", ")}`,
    f.facts.map((x) => `- ${x}`).join("\n"),
    f.explanation,
    f.fix ? `**Fix:** ${f.fix}` : "",
  ];
  return parts.filter((p) => p.trim()).join("\n\n");
}

/** Where a location lands in a review: its file, the row to jump to, and the changed rows a
 * comment on it covers. */
export interface Placed {
  file: number;
  row: number;
  changed: number[];
}

const sameFile = (f: FlatFile, path: string) =>
  f.dto.path === path || f.dto.oldPath === path || f.dto.path.endsWith(`/${path}`);

export function place(files: FlatFile[], loc: Location): Placed | undefined {
  const file = files.findIndex((f) => sameFile(f, loc.path));
  if (file < 0) return undefined;
  const rows = files[file].rows;
  if (rows.length === 0) return undefined;
  const isChanged = (r: number) => rows[r].kind !== "context";
  // Removed rows have no new line, so they take the position of the last one seen.
  let last = 0;
  const at = rows.map((r) => {
    last = r.newLine ?? last;
    return last;
  });
  const { start, end } = loc;
  const inside =
    start === null
      ? rows.map((_, i) => i)
      : at.flatMap((line, i) => (line >= start && line <= (end ?? start) ? [i] : []));
  const changed = inside.filter(isChanged);
  if (changed.length > 0) return { file, row: inside[0], changed };
  // Nothing changed there, so comment on the nearest changed row.
  const target = start ?? 0;
  const nearest = rows
    .map((_, i) => i)
    .filter(isChanged)
    .sort((a, b) => Math.abs(at[a] - target) - Math.abs(at[b] - target))[0];
  if (nearest === undefined) return undefined;
  return { file, row: inside[0] ?? nearest, changed: [nearest] };
}
