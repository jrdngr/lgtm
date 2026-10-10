import type { BotRun } from "../bindings";
import { commands } from "../ipc";
import { Refs } from "./Closing";
import { Markdown } from "./Markdown";

const STATUS = { running: "Fixing…", done: "Done", failed: "Failed", stopped: "Stopped" };

/** Headless fixes of picked findings, newest first, each with the findings it took and the
 * agent's reply as it streams. */
export function Fixes({ run, onGo }: { run: BotRun; onGo(id: string): void }) {
  const fixes = (run.fixes ?? []).map((f, i) => ({ f, i })).reverse();
  return (
    <>
      {fixes.map(({ f, i }) => (
        <section key={i} className={`bots-fix bots-fix-${f.status}`}>
          <div className="bots-fix-head">
            <span className="bots-fix-status">{STATUS[f.status]}</span>
            <Refs ids={f.findings} onGo={onGo} />
            <span className="spacer" />
            {f.status === "running" && (
              <button type="button" onClick={() => commands.stopFix(run.id, i)}>
                Stop
              </button>
            )}
          </div>
          {f.error && <div className="bots-error">{f.error}</div>}
          {f.text ? <Markdown>{f.text}</Markdown> : f.status === "running" && <em>Working…</em>}
        </section>
      ))}
    </>
  );
}
