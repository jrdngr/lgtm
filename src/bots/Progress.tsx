import type { BotRun, Part } from "../bindings";

/** A persona's review in a few words: what it's doing, why it halted, or how much it found. */
function describe(parts: Part[], done: boolean): { state: string; text: string; count?: number } {
  const running = parts.find((p) => p.status === "running");
  if (running) return { state: "running", text: running.activity ?? "starting" };
  const failed = parts.find((p) => p.status === "failed" || p.status === "stopped");
  if (failed) return { state: "halted", text: failed.error ?? failed.status };
  if (parts.length === 0) return { state: "waiting", text: done ? "nothing to do" : "waiting" };
  const out = parts[0].output;
  const count = out?.kind === "review" ? out.findings.length : undefined;
  return { state: "done", text: `${count ?? 0} findings`, count };
}

interface Props {
  run: BotRun;
  /** The persona whose findings are shown, if one is picked. */
  persona?: string;
  onPersona(name: string | undefined): void;
}

/** A filter chip per persona with its finding count, or its state while it isn't done. */
export function Progress({ run, persona, onPersona }: Props) {
  return (
    <>
      {run.personas.map((p) => {
        const parts = run.parts.filter((x) => x.step === "reports" && x.persona === p.name);
        const { state, text, count } = describe(parts, run.done.includes("reports"));
        return (
          <button
            type="button"
            key={p.name}
            className={`bots-chip bots-chip-${state}`}
            aria-pressed={persona === p.name}
            title={`${p.description}\n${text}`}
            onClick={() => onPersona(persona === p.name ? undefined : p.name)}
          >
            {p.name}
            <span className="bots-chip-text">
              {state === "done" ? count : state === "halted" ? "halted" : state}
            </span>
          </button>
        );
      })}
    </>
  );
}
