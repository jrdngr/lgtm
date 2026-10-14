import type { IDockviewPanelProps } from "dockview-react";
import { useRef, useState } from "react";
import type { BotOptions, Finding } from "../bindings";
import { usePr } from "../github/pr";
import { commands } from "../ipc";
import { useCurrentReview } from "../review/current";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { useWorkspace } from "../workspaces/context";
import { FindingCard } from "./FindingCard";
import { covers, Fixes } from "./Fixes";
import { finalFindings, findingsText, history, type Picked, report } from "./findings";
import { runState } from "./personas";
import { MinimalView, PersonaView, SynthesisView } from "./ReportViews";
import { type Act, RunControls } from "./RunControls";
import { refreshRun, runAction, useRun } from "./runs";
import { SettledFold } from "./SettledFold";
import "./bots.css";

type View = "minimal" | "synthesis" | "personas" | "fixes";

const VIEW_NAMES: Record<View, string> = {
  minimal: "Minimal",
  synthesis: "Synthesis",
  personas: "By persona",
  fixes: "Fixes",
};

const ADD: [keyof BotOptions, string][] = [
  ["synthesize", "Add synthesis"],
  ["crossReview", "Add cross-review"],
  ["minimal", "Add minimal report"],
];

const STATE_PILLS = { running: "blue", done: "green", halted: "red" };

const NO_OPTIONS: BotOptions = {
  synthesize: false,
  crossReview: false,
  adversarial: false,
  minimal: false,
};

/** A request to an agent to fix the picked findings. */
function fixPrompt(revset: string, picked: Picked[]): string {
  const findings = findingsText(picked);
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
  /** The last jump, numbered so each one opens the fold it lands in. `id` is set only when
   * the target was already fixed or dismissed, so a card settled later doesn't open its fold. */
  const [jump, setJump] = useState<{ id?: string; n: number }>();
  /** Findings picked to send to the agent, by id. */
  const [checked, setChecked] = useState<Map<string, Picked>>(new Map());
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const canDraft = !!usePr(workspace);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<{ error: boolean; text: string }>();
  const root = useRef<HTMLDivElement>(null);
  if (!run) {
    return (
      <div className="placeholder">{run === null ? "This review run is gone" : "Loading…"}</div>
    );
  }
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

  const act: Act = (f) => runAction(run, f, setError);

  const toggle = (id: string) => {
    const next = new Set(open);
    if (!next.delete(id)) next.add(id);
    setOpen(next);
  };

  // Within this pane, since every run's synthesis numbers its findings from 1.
  const go = (id: string) => {
    setOpen(new Set(open).add(id));
    setJump({ id: stateOf(id) ? id : undefined, n: (jump?.n ?? 0) + 1 });
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

  /** Switches to view `v` and scrolls to finding `id`, showing every persona's findings. */
  const goTo = (v: View, id: string) => {
    if (v === "personas") setPersona(undefined);
    setPicked(v);
    go(id);
  };

  const checkAll = (shown: Picked[]) => {
    const next = new Map(checked);
    for (const x of shown) if (!stateOf(x.f.id ?? "")) next.set(x.f.id ?? "", x);
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
    const text = findingsText([...checked.values()]);
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

  const current = run;
  /** Covered by a Fix headless run that finished, or dismissed. */
  const stateOf = (id: string) =>
    current.fixes?.some((x) => x.status === "done" && covers(current, x, id))
      ? ("fixed" as const)
      : current.dismissed?.includes(id)
        ? ("dismissed" as const)
        : undefined;

  async function dismiss(id: string, dismissed: boolean) {
    const res = await commands.dismissFindings(current.id, [id], dismissed);
    if (res.status === "error") return setNotice({ error: true, text: res.error });
    await refreshRun(current.id);
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
        state={stateOf(id)}
        onDismiss={(d) => dismiss(id, d)}
        canDraft={canDraft}
      />
    );
  };

  /** Cards in order, with fixed and dismissed ones folded at the end. */
  const cards = (list: [Finding, string][]) => {
    const settled = list.filter(([f]) => stateOf(f.id ?? ""));
    const rest = list.filter(([f]) => !stateOf(f.id ?? ""));
    return (
      <>
        {rest.map(([f, s]) => card(f, s))}
        {settled.length > 0 && (
          <SettledFold
            count={settled.length}
            reveal={jump?.id && settled.some(([f]) => f.id === jump.id) ? jump.n : undefined}
          >
            {settled.map(([f, s]) => card(f, s))}
          </SettledFold>
        )}
      </>
    );
  };

  let body: React.ReactNode;
  if (view === "minimal" && r.minimal) {
    body = (
      <MinimalView
        r={r}
        cards={cards}
        checkAll={checkAll}
        sourceOf={sourceOf}
        onGo={(id) => goTo(r.synthesis ? "synthesis" : "personas", id)}
      />
    );
  } else if (view === "fixes") {
    body = <Fixes run={run} onGo={(id) => goTo(id.includes("#") ? "personas" : "synthesis", id)} />;
  } else if (view === "synthesis" && r.synthesis) {
    body = <SynthesisView r={r} cards={cards} checkAll={checkAll} onGo={go} />;
  } else {
    body = (
      <PersonaView
        run={run}
        r={r}
        persona={persona}
        onPersona={setPersona}
        cards={cards}
        checkAll={checkAll}
      />
    );
  }

  const n = finalFindings(r).length;
  return (
    <div className="bots-report" ref={root}>
      <div className="bots-report-bar">
        <span className="bots-report-title">
          Review of <code className="bots-revset">{run.revset}</code>
        </span>
        <code className="bots-commit" title={run.to}>
          {run.to.slice(0, 4)}
        </code>
        <span className={`pill ${STATE_PILLS[state]}`}>
          {state === "done" ? `Done · ${n} ${n === 1 ? "finding" : "findings"}` : label}
        </span>
        {review && review.to !== run.to && (
          <span
            className="pill amber"
            title="Locations and line numbers may not match the open review, and agents read files from the working copy"
          >
            Open review is at {review.to.slice(0, 8)}
          </span>
        )}
        <span className="spacer" />
        <RunControls run={run} act={act} />
        <Menu
          label="Report actions"
          items={ADD.filter(([key]) => !run.active && !run.options[key]).map(([key, text]) => ({
            label: text,
            disabled: key === "crossReview" && run.personas.length < 2,
            title:
              key === "crossReview" && run.personas.length < 2
                ? "Needs two or more personas"
                : undefined,
            onSelect: () =>
              act(() => commands.continueBots(run.id, { ...NO_OPTIONS, [key]: true })),
          }))}
        />
      </div>
      <div className="bots-tabs" role="tablist">
        {views.map((v) => (
          <button
            type="button"
            role="tab"
            key={v}
            aria-selected={v === view}
            onClick={() => setPicked(v)}
          >
            {VIEW_NAMES[v]}
          </button>
        ))}
      </div>
      {notice && (
        <div className={notice.error ? "bots-error bots-notice" : "bots-notice"}>
          {notice.text}
          <IconButton icon="x" label="Dismiss" onClick={() => setNotice(undefined)} />
        </div>
      )}
      {failures.map((p, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: parts never reorder
        <div key={i} className="bots-error">
          {p.persona ?? p.step} failed: {p.error}
        </div>
      ))}
      {error && <div className="bots-error">{error}</div>}
      <div className={`bots-report-body${checked.size > 0 ? " docked" : ""}`}>{body}</div>
      {checked.size > 0 && (
        <div className="bots-picked">
          <b>{checked.size} picked</b>
          <span className="spacer" />
          <button
            type="button"
            disabled={!review}
            title={review ? "Paste them into a terminal's prompt" : "Open a review first"}
            onClick={sendChecked}
          >
            Send to terminal
          </button>
          <button
            type="button"
            className="primary"
            onClick={fixChecked}
            title="Have a headless agent edit the working copy to fix them"
          >
            Fix headless
          </button>
          <IconButton icon="x" label="Clear picks" onClick={() => setChecked(new Map())} />
        </div>
      )}
    </div>
  );
}
