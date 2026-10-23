import type { JjAction } from "../bindings";
import { Busy } from "../ui/Busy";

const RUNNING: Partial<Record<JjAction["kind"], string>> = {
  fetch: "Fetching…",
  push: "Pushing…",
  rebaseOnTrunk: "Rebasing…",
  squash: "Squashing…",
};

/** The jj action running, atop the log. */
export function RunningRow({ action }: { action: JjAction }) {
  return <Busy className="lr-running">{RUNNING[action.kind] ?? "Working…"}</Busy>;
}
