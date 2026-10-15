import { useCallback, useEffect, useState } from "react";
import type { BotRun, Part, Personas, Step } from "../bindings";
import { onSaved } from "../editor/saved";
import { commands } from "../ipc";

/** The persona files, read again whenever `reload` is called or a markdown file is saved in
 * the editor. */
export function usePersonas() {
  const [personas, setPersonas] = useState<Personas>();
  const reload = useCallback(async () => {
    setPersonas(await commands.listPersonas());
  }, []);
  useEffect(() => {
    reload();
    // An edited persona's description or lens may have changed.
    return onSaved((path) => path.endsWith(".md") && reload());
  }, [reload]);
  return { personas, reload };
}

/** Steps a run's options call for, in order. Findings merge to rule on a cross-review, or to
 * give the filter one set from several personas. */
export function steps(run: BotRun): Step[] {
  const { crossReview, suggest } = run.options;
  const synthesizes = crossReview || (suggest && run.personas.length > 1);
  return [
    "reports" as const,
    ...(crossReview ? (["challenges", "rebuttals"] as const) : []),
    ...(synthesizes ? (["synthesis"] as const) : []),
    ...(suggest ? (["minimal"] as const) : []),
  ];
}

/** Whether an agent published the run's findings, rather than lgtm's personas finding them. */
export const published = (run: BotRun) => run.personas.every((p) => !p.profile);

const STEP_NAMES: Record<Step, string> = {
  reports: "reviewing",
  challenges: "cross-reviewing",
  rebuttals: "rebutting",
  synthesis: "synthesizing",
  minimal: "picking fixes",
};

export type RunState = "running" | "done" | "halted";

/** Where a run is, and a few words saying so. */
export function runState(run: BotRun): { state: RunState; label: string } {
  if (run.active) {
    const step = run.parts.filter((p) => p.status === "running").at(-1)?.step ?? "reports";
    if (step !== "reports") return { state: "running", label: `Running · ${STEP_NAMES[step]}` };
    const reports = run.parts.filter((p) => p.step === "reports" && p.status === "done").length;
    return { state: "running", label: `Running · ${reports} of ${run.personas.length} done` };
  }
  if (steps(run).every((s) => run.done.includes(s))) {
    return { state: "done", label: "Done" };
  }
  return { state: "halted", label: "Halted" };
}

/** What a run is doing, which part halted it, or who reviewed. */
export function runDetail(run: BotRun): string {
  const who = (p: Part) => p.persona ?? STEP_NAMES[p.step];
  const running = run.parts.filter((p) => p.status === "running").at(-1);
  if (run.active) return running ? `${who(running)} ${running.activity ?? "starting"}` : "starting";
  const halted = run.parts.find((p) => p.status === "failed" || p.status === "stopped");
  if (halted) return `${who(halted)} ${halted.status}`;
  return run.personas.map((p) => p.name).join(", ");
}
