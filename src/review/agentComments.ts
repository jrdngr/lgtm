import type { AgentCommentDto } from "../bindings";
import { remembered } from "../core/remembered";
import type { Draft } from "./comments";
import { type DisplayLine, type FlatFile, fileSlot, markKey } from "./model";
import { rowsRefs } from "./sendText";
import { unified } from "./snippet";

/** Comments for the agent take negative ids among a review's comment blocks: C1 is -1. */
export const agentId = (number: number) => -number;
export const isAgentId = (id: number) => id < 0;
/** The id of a comment for the agent being written. */
export const NEW_AGENT_COMMENT = Number.MIN_SAFE_INTEGER;

/** Resolved comments for the agent show, folded, in every review and the Comments tab. */
export const showResolved = remembered("lgtm:showResolvedComments", false, [true, false]);

/** A comment for the agent on display lines `start..=end`: their rows in the last file that
 * has any, changed or not. Undefined when there are none. */
export function agentDraftFor(
  files: FlatFile[],
  lines: DisplayLine[],
  start: number,
  end: number,
): Draft | undefined {
  const byFile = new Map<number, Set<number>>();
  for (let i = start; i <= end; i++) {
    const line = lines[i];
    if (line?.kind !== "rows") continue;
    const rows = byFile.get(line.file) ?? new Set<number>();
    for (const r of line.rows) rows.add(r);
    byFile.set(line.file, rows);
  }
  const file = [...byFile.keys()].at(-1);
  if (file === undefined) return undefined;
  const rows = [...(byFile.get(file) ?? [])].sort((a, b) => a - b);
  return { file, rows, snippet: unified(rows.map((r) => files[file].rows[r])) };
}

/** Comment block ids by the slot they show in, for the comments shown: resolved ones only
 * with `resolved`. */
export function agentSlots(
  comments: AgentCommentDto[],
  draft: Draft | undefined,
  resolved: boolean,
): Map<string, number[]> {
  const out = new Map<string, number[]>();
  const add = (key: string, id: number) => out.set(key, [...(out.get(key) ?? []), id]);
  for (const c of comments) {
    if (c.done && !resolved) continue;
    add(c.rows ? markKey(c.file, c.rows.end) : fileSlot(c.file), agentId(c.number));
  }
  if (draft) add(markKey(draft.file, draft.rows[draft.rows.length - 1]), NEW_AGENT_COMMENT);
  return out;
}

/** Two slot maps as one, `a`'s blocks first. */
export function mergeSlots(a: Map<string, number[]>, b: Map<string, number[]>) {
  const out = new Map(a);
  for (const [k, ids] of b) out.set(k, [...(out.get(k) ?? []), ...ids]);
  return out;
}

/** Open comments for the agent per review file. */
export function openCounts(comments: AgentCommentDto[]): Map<number, number> {
  const out = new Map<number, number>();
  for (const c of comments) if (!c.done) out.set(c.file, (out.get(c.file) ?? 0) + 1);
  return out;
}

/** The comment's number, where its code is, and what it says. */
export function agentCommentText(files: FlatFile[], c: AgentCommentDto): string {
  const f = files[c.file];
  const span = c.rows;
  const at =
    f && span
      ? rowsRefs(
          f,
          Array.from({ length: span.end - span.start + 1 }, (_, i) => span.start + i),
        )
      : c.path;
  return `C${c.number} on ${at}\n\n${c.body.trim()}`;
}

/** Its line on the new side, else the old one, for lists. */
export function agentCommentLine(files: FlatFile[], c: AgentCommentDto): number | undefined {
  const row = c.rows && files[c.file]?.rows[c.rows.end];
  return row ? (row.newLine ?? row.oldLine ?? undefined) : undefined;
}
