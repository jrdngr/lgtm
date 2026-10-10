import type { IDockviewPanelProps } from "dockview-react";
import { useRef, useState } from "react";
import type { BotOptions, Finding } from "../bindings";
import { commands } from "../ipc";
import { useCurrentReview } from "../review/current";
import { useWorkspace } from "../workspaces/context";
import { Closing, Refs, Tally } from "./Closing";
import { FindingCard } from "./FindingCard";
import { Fixes } from "./Fixes";
import { findingsById, findingText, history, report, SEVERITIES } from "./findings";
import { Markdown } from "./Markdown";
import { Progress } from "./Progress";
import { runState } from "./personas";
import { refreshRun, useRun } from "./runs";
import "./bots.css";

type View = "minimal" | "synthesis" | "personas" | "fixes";

const VIEW_NAMES: Record<View, string> = {
  minimal: "Minimal",
  synthesis: "Synthesis",
  personas: "By persona",
  fixes: "Fixes",
};

const ADD: [keyof BotOptions, string][] = [
  ["synthesize", "Synthesis"],
  ["crossReview", "Cross-review"],
  ["minimal", "Minimal"],
];

const NO_OPTIONS: BotOptions = {
  synthesize: false,
  crossReview: false,
  adversarial: false,
  minimal: false,
};

function SelectAll({ onClick }: { onClick(): void }) {
  return (
    <button type="button" onClick={onClick} title="Pick every finding shown">
      Pick all
    </button>
  );
}

/** A request to an agent to fix the picked findings. */
function fixPrompt(revset: string, picked: { f: Finding; source: string }[]): string {
  const findings = picked.map(({ f, source }) => findingText(f, source)).join("\n\n---\n\n");
  const n = picked.length;
  return `Fix ${n === 1 ? "this finding" : `these ${n} findings`} from a review of \`${revset}\`. Change only what each one needs.\n\n${findings}`;
}

/** One persona review: its findings as cards, as the minimal set, the synthesis, or each
 * persona's own. */
export function ReportPanel({ params }: IDockviewPanelProps<{ run: number }>) {
  const run = useRun(params.run);
  const [picked, setPicked] = useState<View>();
  const [persona, setPersona] = useState<string>();
  const [open, setOpen] = useState<Set<string>>(new Set());
  /** Findings picked to send to the agent, by id. */
  const [checked, setChecked] = useState<Map<string, { f: Finding; source: string }>>(new Map());
  const review = useCurrentReview(useWorkspace());
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<{ error: boolean; text: string }>();
  const root = useRef<HTMLDivElement>(null);
  if (!run) return <div className="placeholder">This review run is gone</div>;
  const r = report(run);
  const views: View[] = [
    ...(r.minimal ? (["minimal"] as const) : []),
    ...(r.synthesis ? (["synthesis"] as const) : []),
    "personas",
    ...(run.fixes?.length ? (["fixes"] as const) : []),
  ];
  const view = picked && views.includes(picked) ? picked : views[0];
  const { state, label } = runState(run);
  // Persona chips show their own failures, in the By persona view.
  const failures = run.parts.filter(
    (p) => p.status === "failed" && (p.step !== "reports" || !p.persona),
  );
  const sourceOf = (f: Finding) => (f.id?.includes("#") ? f.id.split("#")[0] : "synthesis");

  async function act(f: () => Promise<{ status: string; error?: string }>) {
    const res = await f();
    setError(res.status === "error" ? res.error : undefined);
    if (run) await refreshRun(run.id);
  }

  const toggle = (id: string) => {
    const next = new Set(open);
    if (!next.delete(id)) next.add(id);
    setOpen(next);
  };

  // Within this pane, since every run's synthesis numbers its findings from 1.
  const go = (id: string) => {
    setOpen(new Set(open).add(id));
    requestAnimationFrame(() =>
      root.current
        ?.querySelector(`[data-finding="${CSS.escape(id)}"]`)
        ?.scrollIntoView({ block: "start" }),
    );
  };

  const check = (f: Finding, source: string) => {
    const next = new Map(checked);
    const id = f.id ?? "";
    if (!next.delete(id)) next.set(id, { f, source });
    setChecked(next);
  };

  const checkAll = (shown: { f: Finding; source: string }[]) => {
    const next = new Map(checked);
    for (const x of shown) next.set(x.f.id ?? "", x);
    setChecked(next);
  };

  /** Pastes text into a terminal, and says where it went or why it couldn't. */
  function send(text: string, onSent?: () => void) {
    review?.send(text, onSent, (o) =>
      setNotice(
        "error" in o
          ? { error: true, text: o.error }
          : {
              error: false,
              text: `Pasted into ${o.terminal ? `“${o.terminal}”` : "the terminal"}. Press Enter there to start.`,
            },
      ),
    );
  }

  /** Has a headless agent fix the picked findings, and shows its reply in Fixes. */
  async function fixChecked() {
    if (!run) return;
    const picks = [...checked.values()];
    const text = picks.map(({ f, source }) => findingText(f, source)).join("\n\n---\n\n");
    const res = await commands.fixFindings(run.id, [...checked.keys()], text);
    if (res.status === "error") return setNotice({ error: true, text: res.error });
    setNotice(undefined);
    setChecked(new Map());
    setPicked("fixes");
    await refreshRun(run.id);
  }

  function sendChecked() {
    if (run) send(fixPrompt(run.revset, [...checked.values()]), () => setChecked(new Map()));
  }

  const card = (f: Finding, source: string) => {
    const h = history(r, f);
    const id = f.id ?? "";
    return (
      <FindingCard
        key={id}
        finding={f}
        source={source}
        exchanges={h.exchanges}
        rulings={h.rulings}
        open={open.has(id)}
        onToggle={() => toggle(id)}
        checked={checked.has(id)}
        onCheck={() => check(f, source)}
        onSend={(text) => send(text)}
      />
    );
  };

  const byId = findingsById(r);
  let body: React.ReactNode;
  if (view === "minimal" && r.minimal) {
    const m = r.minimal;
    const kept = m.kept.flatMap((id) => byId.get(id) ?? []);
    const offTask = m.offTask.flatMap((id) => byId.get(id) ?? []);
    body = (
      <>
        <div className="bots-verdict">
          For “{m.task}” · {kept.length} of {r.synthesis?.findings.length ?? byId.size} findings{" "}
          <Tally findings={kept} />
          <SelectAll onClick={() => checkAll(kept.map((f) => ({ f, source: sourceOf(f) })))} />
        </div>
        {kept.map((f) => card(f, sourceOf(f)))}
        {offTask.length > 0 && (
          <section>
            <h3>Off-task critical and high</h3>
            <Refs
              ids={m.offTask}
              onGo={(id) => {
                setPicked(r.synthesis ? "synthesis" : "personas");
                go(id);
              }}
            />
          </section>
        )}
        <section>
          <h3>Recommendations</h3>
          <Markdown>{m.recommendations}</Markdown>
        </section>
      </>
    );
  } else if (view === "fixes") {
    body = (
      <Fixes
        run={run}
        onGo={(id) => {
          setPicked(id.includes("#") ? "personas" : "synthesis");
          go(id);
        }}
      />
    );
  } else if (view === "synthesis" && r.synthesis) {
    const s = r.synthesis;
    body = (
      <>
        <div className="bots-verdict">
          <Markdown>{s.verdict}</Markdown>
          <Tally findings={s.findings} />
          <SelectAll
            onClick={() => checkAll(s.findings.map((f) => ({ f, source: "synthesis" })))}
          />
        </div>
        {s.findings.map((f) => card(f, "synthesis"))}
        <Closing r={r} onGo={go} />
      </>
    );
  } else {
    const who = [...r.byPersona.keys()].filter((p) => !persona || p === persona);
    const findings = who.flatMap((p) => (r.byPersona.get(p) ?? []).map((f) => ({ p, f })));
    findings.sort((a, b) => SEVERITIES.indexOf(a.f.severity) - SEVERITIES.indexOf(b.f.severity));
    body = (
      <>
        {persona && r.verdicts.get(persona) && (
          <div className="bots-verdict">
            <Markdown>{r.verdicts.get(persona) ?? ""}</Markdown>
          </div>
        )}
        <div className="bots-verdict">
          <Tally findings={findings.map((x) => x.f)} />
          <SelectAll onClick={() => checkAll(findings.map(({ p, f }) => ({ f, source: p })))} />
        </div>
        {findings.map(({ p, f }) => card(f, p))}
      </>
    );
  }

  return (
    <div className="bots-report" ref={root}>
      <div className="bots-report-bar">
        <code>{run.revset}</code> @ <code>{run.to.slice(0, 8)}</code>
        <span className="bots-views" role="tablist">
          {views.map((v) => (
            <button
              type="button"
              role="tab"
              key={v}
              aria-selected={v === view}
              className={v === view ? "selected" : undefined}
              onClick={() => setPicked(v)}
            >
              {VIEW_NAMES[v]}
            </button>
          ))}
        </span>
        <span className={`bots-run-meta bots-state-${state}`}>{label}</span>
        {review && review.to !== run.to && (
          <span
            className="bots-run-meta bots-state-halted"
            title="Locations and line numbers may not match the open review, and agents read files from the working copy"
          >
            open review is at <code>{review.to.slice(0, 8)}</code>
          </span>
        )}
        <span className="spacer" />
        {run.active ? (
          <button type="button" onClick={() => act(() => commands.stopBots(run.id))}>
            Stop
          </button>
        ) : (
          <>
            {state === "halted" && (
              <button
                type="button"
                onClick={() => act(() => commands.continueBots(run.id, run.options))}
              >
                Retry
              </button>
            )}
            {ADD.filter(([key]) => !run.options[key]).map(([key, text]) => (
              <button
                type="button"
                key={key}
                disabled={key === "crossReview" && run.personas.length < 2}
                onClick={() =>
                  act(() => commands.continueBots(run.id, { ...NO_OPTIONS, [key]: true }))
                }
              >
                + {text}
              </button>
            ))}
          </>
        )}
      </div>
      {view === "personas" && <Progress run={run} persona={persona} onPersona={setPersona} />}
      {checked.size > 0 && (
        <div className="bots-picked">
          {checked.size} picked
          <button
            type="button"
            onClick={fixChecked}
            title="Have a headless agent edit the working copy to fix them"
          >
            Fix headless
          </button>
          <button
            type="button"
            disabled={!review}
            title={review ? "Paste them into a terminal's prompt" : "Open a review first"}
            onClick={sendChecked}
          >
            Send to terminal
          </button>
          <button type="button" onClick={() => setChecked(new Map())}>
            Clear
          </button>
        </div>
      )}
      {notice && (
        <div className={notice.error ? "bots-error bots-notice" : "bots-notice"}>
          {notice.text}
          <button type="button" onClick={() => setNotice(undefined)} title="Dismiss">
            ✕
          </button>
        </div>
      )}
      {failures.map((p, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: parts never reorder
        <div key={i} className="bots-error">
          {p.persona ?? p.step} failed: {p.error}
        </div>
      ))}
      {error && <div className="bots-error">{error}</div>}
      <div className="bots-report-body">{body}</div>
    </div>
  );
}
