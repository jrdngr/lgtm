import type { ArchDto, SystemDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { commands } from "../ipc";

export type GenState = "fresh" | "stale" | "missing";

export type GenKey = "description" | "assessment";

export interface GenItem {
  key: GenKey;
  label: string;
  state: GenState;
}

type Prompt = { status: "ok"; data: string } | { status: "error"; error: string };

/** What Generate offers: the description, and with a review, its assessment. */
export function genItems(map: SystemDto | undefined, review?: { arch?: ArchDto }): GenItem[] {
  const description: GenState = !map?.description
    ? "missing"
    : map.stale.length
      ? "stale"
      : "fresh";
  const items: GenItem[] = [{ key: "description", label: "Description", state: description }];
  if (review) {
    const a = review.arch?.assessment;
    const state: GenState = !a ? "missing" : a.stale ? "stale" : "fresh";
    items.push({ key: "assessment", label: "Assessment", state });
  }
  return items;
}

/** One prompt asking for each of `keys` in turn: the description first, so the assessment
 * sees its names. */
export async function generatePrompt(
  workspace: WorkspaceName,
  review: number | undefined,
  keys: GenKey[],
): Promise<Prompt> {
  const parts: Prompt[] = [];
  if (keys.includes("description")) parts.push(await commands.describePrompt(workspace));
  if (keys.includes("assessment") && review !== undefined)
    parts.push(await commands.assessPrompt(review));
  const failed = parts.find((p) => p.status === "error");
  if (failed) return failed;
  const texts = parts.map((p) => (p.status === "ok" ? p.data : ""));
  if (texts.length < 2) return { status: "ok", data: texts[0] ?? "" };
  return {
    status: "ok",
    data: texts.map((t, i) => `Part ${i + 1} of ${texts.length}. ${t}`).join("\n\n---\n\n"),
  };
}
