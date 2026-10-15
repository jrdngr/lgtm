import type { BotRun, Fix } from "../bindings";
import { commands } from "../ipc";
import { Busy } from "../ui/Busy";
import { Markdown } from "./Markdown";
import { Refs } from "./Refs";

const STATUS = { running: "Fixing…", done: "Done", failed: "Failed", stopped: "Stopped" };

/** Whether `fix` took finding `id`. A synthesis id from an earlier synthesis names a finding
 * that's gone. */
export function covers(run: BotRun, fix: Fix, id: string) {
  const current = id.includes("#") || (fix.synthesis ?? 0) === (run.synthesis ?? 0);
  return current && fix.findings.includes(id);
}

/** Headless fixes of picked findings, newest first, each with the findings it took and the
 * agent's reply as it streams. */
export function Fixes({ run, onGo }: { run: BotRun; onGo(id: string): void }) {
  const fixes = (run.fixes ?? []).map((f, i) => ({ f, i })).reverse();
  return (
    <>
      {fixes.map(({ f, i }) => (
        <section key={i} className={`bots-fix bots-fix-${f.status}`}>
          <div className="bots-fix-head">
            <span className="bots-fix-status">
              {f.status === "running" ? <Busy>{STATUS.running}</Busy> : STATUS[f.status]}
            </span>
            <Refs ids={f.findings} onGo={onGo} gone={(id) => !covers(run, f, id)} />
            <span className="spacer" />
            {f.status === "running" && (
              <button type="button" onClick={() => commands.stopFix(run.id, i)}>
                Stop
              </button>
            )}
          </div>
          {f.error && <div className="bots-error">{f.error}</div>}
          {f.text ? <Markdown>{f.text}</Markdown> : f.status === "running" && <Busy>Working…</Busy>}
        </section>
      ))}
    </>
  );
}
