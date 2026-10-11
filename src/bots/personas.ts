import { useCallback, useEffect, useState } from "react";
import type { BotOptions, BotRun, Personas, Step } from "../bindings";
import { onSaved } from "../editor/saved";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";

/** The workspace's persona files, read again whenever `reload` is called or a markdown file
 * is saved in the editor. */
export function usePersonas(workspace: WorkspaceName) {
  const [personas, setPersonas] = useState<Personas>();
  const [error, setError] = useState<string>();
  const reload = useCallback(async () => {
    const r = await commands.listPersonas(workspace);
    if (r.status === "ok") setPersonas(r.data);
    setError(r.status === "error" ? r.error : undefined);
  }, [workspace]);
  useEffect(() => {
    reload();
    // An edited persona's description or lens may have changed.
    return onSaved((path) => path.endsWith(".md") && reload());
  }, [reload]);
  return { personas, error, reload };
}

/** Steps a run's options call for, in order. */
export function steps(o: BotOptions): Step[] {
  return [
    "reports" as const,
    ...(o.crossReview ? (["challenges", "rebuttals"] as const) : []),
    ...(o.synthesize ? (["synthesis"] as const) : []),
    ...(o.minimal ? (["minimal"] as const) : []),
  ];
}

const STEP_NAMES: Record<Step, string> = {
  reports: "reviewing",
  challenges: "cross-reviewing",
  rebuttals: "rebutting",
  synthesis: "synthesizing",
  minimal: "picking the minimal set",
};

export type RunState = "running" | "done" | "halted";

/** Where a run is, and a few words saying so. */
export function runState(run: BotRun): { state: RunState; label: string } {
  if (run.active) {
    const step = run.parts.filter((p) => p.status === "running").at(-1)?.step ?? "reports";
    if (step !== "reports") return { state: "running", label: STEP_NAMES[step] };
    const reports = run.parts.filter((p) => p.step === "reports" && p.status === "done").length;
    return { state: "running", label: `reviewing ${reports}/${run.personas.length}` };
  }
  if (steps(run.options).every((s) => run.done.includes(s))) {
    return { state: "done", label: "done" };
  }
  const failed = run.parts.some((p) => p.status === "failed");
  return { state: "halted", label: failed ? "failed" : "stopped" };
}
