import type { ChunkVerdict } from "../bindings";
import { commands, unwrap } from "../ipc";
import type { CurrentReview } from "../review/current";
import { type ChunkState, RISK_NAMES, rangesText } from "./model";
import { setGuided } from "./store";

/** Gives `states` verdict `verdict`, or takes theirs back with `null`. */
export async function giveVerdict(
  review: CurrentReview,
  states: ChunkState[],
  verdict: ChunkVerdict | null,
  note = "",
) {
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
