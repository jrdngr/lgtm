import type { Action, BotRun, Finding, Location, Minimal, Severity } from "../bindings";
import type { FlatFile } from "../review/model";

export const SEVERITIES: Severity[] = ["critical", "high", "medium", "low", "info"];

/** Short names for the flat severity tags. */
export const SEVERITY_NAMES: Record<Severity, string> = {
  critical: "critical",
  high: "high",
  medium: "med",
  low: "low",
  info: "info",
};

/** Emoji badges, for finding text sent out as comments and prompts. */
export const BADGES: Record<Severity, string> = {
  critical: "🔴 CRITICAL",
  high: "🟠 HIGH",
  medium: "🟡 MEDIUM",
  low: "🔵 LOW",
  info: "⚪ INFO",
};

/** Everything the report pane shows of a run, gathered from its parts. */
export interface Report {
  /** Each reviewer's findings, by name. */
  byPersona: Map<string, Finding[]>;
  verdicts: Map<string, string>;
  minimal?: Minimal;
}

export function report(run: BotRun): Report {
  const byPersona = new Map<string, Finding[]>();
  const verdicts = new Map<string, string>();
  let minimal: Minimal | undefined;
  for (const { persona, output } of run.parts) {
    if (output.kind === "minimal") minimal = output;
    else {
      byPersona.set(persona ?? "", output.findings);
      verdicts.set(persona ?? "", output.verdict);
    }
  }
  return { byPersona, verdicts, minimal };
}

/** Every reviewer's findings. */
export function allFindings(r: Report): Finding[] {
  return [...r.byPersona.values()].flat();
}

/** The verdict on the whole change: a lone reviewer's. */
export function verdictOf(r: Report): string | undefined {
  return r.verdicts.size === 1 ? [...r.verdicts.values()][0] : undefined;
}

/** A finding's id as the report shows it: `#3`, or with a lone reviewer, `#3` for `sec#3`. */
export function refLabel(run: BotRun, id: string): string {
  if (!id.includes("#")) return `#${id}`;
  return run.personas.length === 1 ? `#${id.split("#")[1]}` : id;
}

/** The reviewer that raised a finding. */
export const raiser = (f: Finding) => f.id?.split("#")[0] ?? "";

/** Suggested fixes as a request to an agent, each with where its findings are. */
export function actionsText(revset: string, actions: Action[], byId: Map<string, Finding>) {
  const n = actions.length;
  const list = actions.map((a, i) => {
    const where = a.closes
      .flatMap((id) => byId.get(id)?.locations ?? [])
      .map((l) => `\`${locationText(l)}\``);
    return `${i + 1}. ${a.action}${where.length ? `\n   ${[...new Set(where)].join(", ")}` : ""}`;
  });
  return `Make ${n === 1 ? "this fix" : `these ${n} fixes`} from a review of \`${revset}\`. Change only what each one needs.\n\n${list.join("\n")}`;
}

/** Findings by id, for resolving references. */
export function findingsById(r: Report): Map<string, Finding> {
  const out = new Map<string, Finding>();
  for (const fs of r.byPersona.values()) for (const f of fs) if (f.id) out.set(f.id, f);
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

/** A finding picked to send, with whose finding it is. */
export interface Picked {
  f: Finding;
  source: string;
}

/** Picked findings as markdown, one after another. */
export const findingsText = (picks: Picked[]) =>
  picks.map(({ f, source }) => findingText(f, source)).join("\n\n---\n\n");

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
