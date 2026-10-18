import type {
  ArchDto,
  ArchRange,
  HistoryDto,
  IntentDto,
  LayerDto,
  LookDto,
  RowDto,
} from "../../bindings";
import type { CurrentReview } from "../../review/current";
import { markKey, needsReview } from "../../review/model";
import { stateDone } from "../../review/rowState";

/** 1–2 red, 3 amber, 4–5 green. */
export const meterTone = (score: number) => (score >= 4 ? "hi" : score === 3 ? "mid" : "lo");

/** Flat rows of a file that fall in lines `start` to `end`: new-side lines, or old-side in a
 * file with no new side. A removed row sits just after the new line before it. */
export function rowsInLines(rows: RowDto[], start: number, end: number): number[] {
  const oldSide = rows.every((r) => r.newLine === null);
  const out: number[] = [];
  let lastNew = 0;
  rows.forEach((r, i) => {
    const at = oldSide ? r.oldLine : (r.newLine ?? lastNew + 0.5);
    if (r.newLine !== null) lastNew = r.newLine;
    if (at !== null && start <= at && at <= end) out.push(i);
  });
  return out;
}

type Marks = Pick<CurrentReview, "rowState" | "done"> & {
  files: { rows: RowDto[] }[];
};

/** Share of a layer's rows that are reviewed, or undefined when none need review. */
export function layerDone(review: Marks, layer: LayerDto): number | undefined {
  const rows = new Map<string, [number, number]>();
  for (const r of layer.ranges) {
    const f = r.file === null ? undefined : review.files[r.file];
    if (r.file === null || !f) continue;
    for (const i of rowsInLines(f.rows, r.start, r.end)) {
      if (needsReview(f.rows[i])) rows.set(markKey(r.file, i), [r.file, i]);
    }
  }
  if (rows.size === 0) return undefined;
  const done = [...rows.values()].filter(([f, i]) => stateDone(review.rowState(f, i), review.done));
  return done.length / rows.size;
}

/** The range a History click picks, with `list` oldest first. A click shows that commit's
 * delta, or the whole review when it's already shown. Shift-click with one commit shown
 * compares the older of the two to the newer. */
export function clickRange(
  list: HistoryDto[],
  range: ArchRange | undefined,
  h: HistoryDto,
  shift: boolean,
): ArchRange | undefined {
  const own = { from: h.parent, to: h.commitId };
  const [shown] = pickedCommits(list, range);
  const single = pickedCommits(list, range).length === 1;
  if (shift && single && shown !== h.commitId) {
    const at = (id: string) => list.findIndex((c) => c.commitId === id);
    const [older, newer] = at(shown) < at(h.commitId) ? [shown, h.commitId] : [h.commitId, shown];
    return { from: older, to: newer };
  }
  return single && shown === h.commitId ? undefined : own;
}

/** The commits `range` picks in History: one for its own delta, two for a comparison. */
export function pickedCommits(list: HistoryDto[], range: ArchRange | undefined): string[] {
  if (!range) return [];
  const own = list.some((c) => c.commitId === range.to && c.parent === range.from);
  return own || range.from === null ? [range.to] : [range.from, range.to];
}

/** What needs a look before merging: broken intents and open high Look items. */
export const archBadge = (arch: ArchDto | undefined) =>
  arch
    ? arch.intents.filter((i) => i.state === "broken").length +
      arch.look.filter((l) => l.severity === "high" && !l.checked).length
    : 0;

const SEVERITY = ["high", "medium", "low"];

/** Look items, worst first. */
export const bySeverity = (look: LookDto[]) =>
  [...look].sort((a, b) => SEVERITY.indexOf(a.severity) - SEVERITY.indexOf(b.severity));

export const STATE_LABEL = { broken: "Broken", open: "Open", met: "Met", tag: "Tag" } as const;

/** An intent's two lines: what it's about, and what it says. `name` maps a component id to
 * its name. */
export function intentText(it: IntentDto, name: (id: string) => string): [string, string] {
  switch (it.kind) {
    case "wrong":
      return [`${name(it.from)} → ${name(it.to)}`, it.reason ? `Wrong · ${it.reason}` : "Wrong"];
    case "move":
      return [`${it.symbol ?? it.path} → ${name(it.to)}`, `Belongs in ${name(it.to)}`];
    case "risk":
      return [name(it.component), "Risk"];
    case "trust":
      return [name(it.component), "Trust boundary"];
    case "note":
      return [name(it.component), it.text];
  }
}
