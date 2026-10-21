import type { ChunkVerdict, MarkColor } from "../bindings";
import { commands, unwrap } from "../ipc";
import type { CurrentReview } from "../review/current";
import type { MarkSetup } from "../review/marks";
import { type ChunkState, changedRows, RISK_NAMES, rangesText } from "./model";
import { setGuided } from "./store";

/** The color a verdict marks a chunk's changed lines with in the Review tab, if any. */
function markColor(setup: MarkSetup, verdict: ChunkVerdict): MarkColor | undefined {
  if (verdict === "approved") return setup.primary ?? setup.shown.find((s) => s.done)?.color;
  if (verdict === "skipped")
    return setup.slots.find((s) => s.label.trim().toLowerCase() === "skipped")?.color ?? "gray";
  return undefined;
}

/** Gives `states` verdict `verdict`, or takes theirs back with `null`. Approve and Skip also mark
 * their changed lines, so the Review tab's progress agrees. */
export async function giveVerdict(
  review: CurrentReview,
  setup: MarkSetup,
  states: ChunkState[],
  verdict: ChunkVerdict | null,
  note = "",
) {
  const color = verdict && markColor(setup, verdict);
  const rows = color ? states.flatMap((s) => changedRows(review.files, s.segments)) : [];
  if (color && rows.length)
    review.apply(await unwrap(commands.markRows(review.id, rows, color, [])));
  const sets = states.map((s) => ({ chunk: s.chunk.id, verdict, text: s.text, note }));
  setGuided(review.id, await unwrap(commands.setGuidedVerdicts(review.id, sets)));
}

const about = (s: ChunkState) =>
  `the "${s.chunk.title}" chunk (${RISK_NAMES[s.chunk.risk]} risk, id \`${s.chunk.id}\`) of the guided review, at:\n${rangesText(s.chunk.ranges)}`;

/** What Change sends the agent: the chunk and what to do about it. */
export const changeText = (s: ChunkState, note: string) =>
  `Change ${about(s)}\n\n${note.trim()}\n\nThen update the guided review, keeping that chunk's id.`;

/** What Ask sends the agent, for the person to finish with their question. */
export const askText = (s: ChunkState) => `About ${about(s)}\n\n`;
