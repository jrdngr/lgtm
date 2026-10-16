import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useRef, useState } from "react";
import type { Finding } from "../bindings";
import { usePr } from "../github/pr";
import { commands } from "../ipc";
import { useCurrentReview } from "../review/current";
import { useSendText } from "../review/useSendText";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { useWorkspace } from "../workspaces/context";
import { FindingCard } from "./FindingCard";
import { FindingsView } from "./FindingsView";
import {
  actionsText,
  allFindings,
  findingsById,
  findingsText,
  type Picked,
  report,
} from "./findings";
import { reportIdItems, reportLabel } from "./reportId";
import { refreshRun, useRun } from "./runs";
import { SettledFold } from "./SettledFold";
import { SuggestedView } from "./SuggestedView";
import "./bots.css";

type View = "suggested" | "findings";

const VIEW_NAMES: Record<View, string> = {
  suggested: "Suggested fixes",
  findings: "Findings",
};

/** A request to an agent to fix the picked findings. */
function fixPrompt(revset: string, picked: Picked[]): string {
  const findings = findingsText(picked);
  const n = picked.length;
  return `Fix ${n === 1 ? "this finding" : `these ${n} findings`} from a review of \`${revset}\`. Change only what each one needs.\n\n${findings}`;
}

/** One report: the fixes its task needs, and every finding. */
export function ReportPanel({ params, api }: IDockviewPanelProps<{ run: number }>) {
  const run = useRun(params.run);
  const reportId = run && reportLabel(run);
  useEffect(() => {
    if (reportId) api.setTitle(`Report ${reportId}`);
  }, [api, reportId]);
  const [picked, setPicked] = useState<View>();
  const [persona, setPersona] = useState<string>();
  const [open, setOpen] = useState<Set<string>>(new Set());
  /** The last jump, numbered so each one opens the fold it lands in. `id` is set only when
   * the target was already dismissed, so a card dismissed later doesn't open its fold. */
  const [jump, setJump] = useState<{ id?: string; n: number }>();
  /** Findings picked to send to the agent, by id. */
  const [checked, setChecked] = useState<Map<string, Picked>>(new Map());
  /** Suggested fixes left out of Send, by index, for the suggestions they were for. */
  const [skipped, setSkipped] = useState<{ of?: string; i: Set<number> }>({ i: new Set() });
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const canDraft = !!usePr(workspace);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<{ error: boolean; text: string }>();
  const root = useRef<HTMLDivElement>(null);
  const { sendText, picker } = useSendText(workspace, setError, () => {});
  if (!run) {
    return <div className="placeholder">{run === null ? "This report is gone" : "Loading…"}</div>;
  }
  const r = report(run);
  const views: View[] = r.minimal ? ["suggested", "findings"] : ["findings"];
  const view = picked && views.includes(picked) ? picked : views[0];
  const fixes = r.minimal?.fixes ?? [];
  const suggestions = r.minimal && [r.minimal.task, ...fixes.map((a) => a.action)].join("\n");
  const skip = skipped.of === suggestions ? skipped.i : new Set<number>();
  const sending = fixes.filter((_, i) => !skip.has(i));
  const isDismissed = (id: string) => !!run.dismissed?.includes(id);

  const toggle = (id: string) => {
    const next = new Set(open);
    if (!next.delete(id)) next.add(id);
    setOpen(next);
  };

  // Within this pane, since every report numbers its findings from 1.
  const go = (id: string) => {
    setOpen(new Set(open).add(id));
    setJump({ id: isDismissed(id) ? id : undefined, n: (jump?.n ?? 0) + 1 });
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

  /** Shows finding `id` in Findings, with every reviewer's findings. */
  const goTo = (id: string) => {
    setPersona(undefined);
    setPicked("findings");
    go(id);
  };

  const skipFix = (i: number) => {
    const next = new Set(skip);
    if (!next.delete(i)) next.add(i);
    setSkipped({ of: suggestions || undefined, i: next });
  };

  /** Pastes text into a terminal, and says where it went or why it couldn't. */
  function send(text: string, onSent?: () => void) {
    sendText(text, onSent, (o) =>
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

  const fixesText = () => actionsText(run.revset, sending, findingsById(r));

  function sendChecked() {
    if (run) send(fixPrompt(run.revset, [...checked.values()]), () => setChecked(new Map()));
  }

  async function dismiss(id: string, dismissed: boolean) {
    if (!run) return;
    const res = await commands.dismissFindings(run.id, [id], dismissed);
    if (res.status === "error") return setNotice({ error: true, text: res.error });
    await refreshRun(run.id);
  }

  const card = (f: Finding, source: string) => {
    const id = f.id ?? "";
    return (
      <FindingCard
        key={id}
        finding={f}
        source={source}
        open={open.has(id)}
        onToggle={() => toggle(id)}
        checked={checked.has(id)}
        onCheck={() => check(f, source)}
        onSend={(text) => send(text)}
        dismissed={isDismissed(id)}
        onDismiss={(d) => dismiss(id, d)}
        canDraft={canDraft}
      />
    );
  };

  /** Cards in order, with dismissed ones folded at the end. */
  const cards = (list: [Finding, string][]) => {
    const settled = list.filter(([f]) => isDismissed(f.id ?? ""));
    const rest = list.filter(([f]) => !isDismissed(f.id ?? ""));
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

  const body =
    view === "findings" ? (
      <FindingsView
        run={run}
        r={r}
        persona={persona}
        onPersona={setPersona}
        cards={cards}
        isOpen={(id) => open.has(id)}
        onOpen={toggle}
        isChecked={(id) => checked.has(id)}
        onCheck={check}
      />
    ) : (
      <SuggestedView run={run} r={r} skipped={skip} onSkip={skipFix} onGo={goTo} />
    );

  const n = allFindings(r).length;
  const dock =
    view === "suggested"
      ? sending.length > 0 && (
          <div className="bots-picked">
            <b>
              {sending.length} of {fixes.length} fixes
            </b>
            <span className="spacer" />
            <button type="button" className="primary" onClick={() => send(fixesText())}>
              Send to terminal
            </button>
          </div>
        )
      : view === "findings" &&
        checked.size > 0 && (
          <div className="bots-picked">
            <b>{checked.size} picked</b>
            <span className="spacer" />
            <button type="button" className="primary" onClick={sendChecked}>
              Send to terminal
            </button>
            <IconButton icon="x" label="Clear picks" onClick={() => setChecked(new Map())} />
          </div>
        );
  return (
    <div className="bots-report" ref={root}>
      <div className="bots-report-bar">
        <code className="bots-report-id">{reportId}</code>
        <span className="bots-report-title">
          Review of <code className="bots-revset">{run.revset}</code>
        </span>
        <code className="bots-commit">{run.to.slice(0, 4)}</code>
        <span className="pill green">
          Published · {n} {n === 1 ? "finding" : "findings"}
        </span>
        {review && review.to !== run.to && (
          <span className="pill amber">Open review is at {review.to.slice(0, 8)}</span>
        )}
        <span className="spacer" />
        <Menu label="Report actions" items={reportIdItems(run, setError)} />
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
            {v === "suggested" && r.minimal && (
              <span className="bots-tab-count">{fixes.length}</span>
            )}
            {v === "findings" && <span className="bots-tab-count">{n}</span>}
          </button>
        ))}
      </div>
      {notice && (
        <div className={notice.error ? "bots-error bots-notice" : "bots-notice"}>
          {notice.text}
          <IconButton icon="x" label="Dismiss" onClick={() => setNotice(undefined)} />
        </div>
      )}
      {error && <div className="bots-error">{error}</div>}
      <div className={`bots-report-body${dock ? " docked" : ""}`}>{body}</div>
      {dock}
      {picker}
    </div>
  );
}
