import type { BotRun, Part, Step } from "../bindings";

/** A part's state in a few words: what it's doing, how it ended, or what it found. */
function describe(parts: Part[], step: Step, done: boolean): { state: string; text: string } {
  const running = parts.find((p) => p.status === "running");
  if (running) return { state: "running", text: running.activity ?? "starting" };
  const failed = parts.find((p) => p.status === "failed" || p.status === "stopped");
  if (failed) return { state: "halted", text: failed.error ?? failed.status };
  if (parts.length === 0) return { state: "waiting", text: done ? "nothing to do" : "waiting" };
  const out = parts[0].output;
  if (step === "reports" && out?.kind === "review") {
    return { state: "done", text: `${out.findings.length} findings` };
  }
  return { state: "done", text: "done" };
}

interface Props {
  run: BotRun;
  /** The persona the By persona view shows, if one is picked. */
  persona?: string;
  onPersona(name: string | undefined): void;
}

/** One chip per persona with its state. Clicking one shows only its findings. */
export function Progress({ run, persona, onPersona }: Props) {
  return (
    <div className="bots-progress">
      {run.personas.map((p) => {
        const parts = run.parts.filter((x) => x.step === "reports" && x.persona === p.name);
        const { state, text } = describe(parts, "reports", run.done.includes("reports"));
        return (
          <button
            type="button"
            key={p.name}
            className={`bots-chip bots-chip-${state}${persona === p.name ? " selected" : ""}`}
            title={`${p.description}\n${text}`}
            onClick={() => onPersona(persona === p.name ? undefined : p.name)}
          >
            {p.name} <span className="bots-chip-text">{text}</span>
          </button>
        );
      })}
    </div>
  );
}
