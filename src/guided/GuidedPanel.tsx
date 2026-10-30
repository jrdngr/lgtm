import { useState } from "react";
import type { ChunkRisk, ExcludeReason } from "../bindings";
import { useSavedSet } from "../core/savedState";
import { useWorkspace } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import { type CurrentReview, useCurrentReview } from "../review/current";
import { Diffstat } from "../review/Diffstat";
import { sumStats } from "../review/lineStat";
import { useSendText } from "../review/useSendText";
import { run } from "../services/pathMenu";
import { PipeButton } from "../terminal/PipeButton";
import { keepSubmit } from "../terminal/registry";
import { type CtxEntry, contextMenu } from "../ui/contextMenu";
import { GroupHead } from "../ui/GroupHead";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { giveVerdict } from "./actions";
import {
  type ChunkState,
  type ExcludedState,
  type Progress,
  REASON_NAMES,
  RISK_NAMES,
  RISKS,
  threadsIn,
  threadTitle,
} from "./model";
import { openGuided, type Pick, samePick, usePick } from "./store";
import { useGuidedView } from "./useGuidedView";
import "../review/comments.css";
import "./guided.css";

/** A verdict's glyph in a chunk row. */
function VerdictMark({ s }: { s: ChunkState }) {
  if (s.changed) return <span className="gd-v changed">↻</span>;
  if (s.verdict?.verdict === "approved") return <span className="gd-v approved">✓</span>;
  if (s.verdict?.verdict === "change") return <span className="gd-v change">↺</span>;
  if (s.verdict?.verdict === "skipped") return <span className="gd-v skipped">–</span>;
  return <span className="gd-v" />;
}

/** Unapproved lines by risk, then approved and skipped ones, as one bar. */
function ProgressBar({ p }: { p: Progress }) {
  const total = Math.max(p.total, 1);
  const parts: [string, number][] = [
    ...RISKS.map((r): [string, number] => [`risk-${r}`, p.open[r]]),
    ["approved", p.approved],
    ["skipped", p.skipped],
  ];
  const done = p.total ? Math.round(((p.approved + p.skipped) / p.total) * 100) : 0;
  return (
    <div className="gd-progress">
      <div className="gd-bar">
        {parts.map(
          ([cls, n]) =>
            n > 0 && <i key={cls} className={cls} style={{ width: `${(n / total) * 100}%` }} />,
        )}
      </div>
      <span className="gd-pct">{done}%</span>
    </div>
  );
}

interface RowProps {
  pick: Pick;
  picked: Pick | undefined;
  onPick(pick: Pick): void;
  menu?(): CtxEntry[];
  className?: string;
  children: React.ReactNode;
}

function Row({ pick, picked, onPick, menu, className, children }: RowProps) {
  return (
    <button
      type="button"
      className={`gd-row${className ? ` ${className}` : ""}`}
      aria-current={samePick(picked, pick)}
      onClick={() => onPick(pick)}
      onContextMenu={menu && contextMenu(menu)}
    >
      {children}
    </button>
  );
}

function chunkMenu(review: CurrentReview, states: ChunkState[]): CtxEntry[] {
  const give = (v: Parameters<typeof giveVerdict>[2]) => run(giveVerdict(review, states, v));
  return [
    { label: states.length > 1 ? "Approve All" : "Approve", onSelect: () => give("approved") },
    { label: states.length > 1 ? "Skip All" : "Skip", onSelect: () => give("skipped") },
    states.some((s) => s.verdict) && { label: "Clear Verdict", onSelect: () => give(null) },
  ];
}

/** The current review's guided review: its progress, its chunks by risk, the code PR threads
 * are on, and what the agent left out. Picking one shows it in the Guided Review tab. */
export function GuidedPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const { g, error, view } = useGuidedView(review);
  const picked = usePick(workspace);
  const [sendError, setSendError] = useState<string>();
  const { sendText, picker } = useSendText(workspace, setSendError, () => {}, "background");
  const [shut, setShut] = useSavedSet(`guided-shut:${workspace ?? ""}`);
  const toggle = (key: string) =>
    setShut((s) => {
      const next = new Set(s);
      if (!next.delete(key)) next.add(key);
      return next;
    });
  // Risk groups start open; Tests, Excluded and each reason start folded, so theirs are kept
  // as `:open` keys.
  const open = (key: string) => !shut.has(key);
  const isShut = (key: string) => shut.has(key);
  const pick = (p: Pick) => openGuided(workspace, p);

  function generate() {
    if (!review) return;
    keepSubmit(unwrap(commands.guidedPrompt(review.id)))
      .then((text) => sendText(text))
      .catch((e) => setSendError(String(e)));
  }

  const header = (
    <SectionHeader
      title="Guided"
      meta={g?.stale ? <span className="pill amber">Stale</span> : undefined}
    >
      {review && (
        <PipeButton background onClick={generate}>
          {g ? "Update" : "Generate"}
        </PipeButton>
      )}
    </SectionHeader>
  );
  const problem = sendError ?? error;
  if (!review || !view) {
    return (
      <div className="side-split">
        {header}
        {problem && <div className="review-error">{problem}</div>}
        <Empty>{review ? (g === null ? "No guided review" : "Loading…") : "No review open"}</Empty>
        {picker}
      </div>
    );
  }

  const chunkRow = (s: ChunkState) => {
    const threads = threadsIn(review.comments, s.segments).length;
    return (
      <Row
        key={s.chunk.id}
        pick={{ kind: "chunk", id: s.chunk.id }}
        picked={picked}
        onPick={pick}
        menu={() => chunkMenu(review, [s])}
        className={s.verdict?.verdict === "skipped" && !s.changed ? "skipped" : undefined}
      >
        <VerdictMark s={s} />
        <span className="gd-title">{s.chunk.title}</span>
        {threads > 0 && <span className="gd-threads">{threads}</span>}
        <Diffstat stat={s.stat} />
      </Row>
    );
  };

  const riskGroup = (risk: ChunkRisk) => {
    const states = view.states.filter((s) => s.chunk.risk === risk);
    if (!states.length) return null;
    const stat = sumStats(states.map((s) => s.stat));
    return (
      <div key={risk}>
        <GroupHead
          name={
            <span className="gd-group">
              <span className={`gd-dot risk-${risk}`} />
              {RISK_NAMES[risk]}
            </span>
          }
          count={
            <>
              {states.length}
              <Diffstat stat={stat} />
            </>
          }
          shut={isShut(risk)}
          onToggle={() => toggle(risk)}
          onContextMenu={contextMenu(() => chunkMenu(review, states))}
        />
        {open(risk) && states.map(chunkRow)}
      </div>
    );
  };

  const partRow = (e: ExcludedState) => (
    <Row
      key={e.index}
      pick={{ kind: "excluded", index: e.index }}
      picked={picked}
      onPick={pick}
      className="sub"
    >
      <span className="gd-title mono">{e.part.path}</span>
      <Diffstat stat={e.stat} />
    </Row>
  );

  const reasons = [...new Set(view.excluded.map((e) => e.part.reason))] as ExcludeReason[];

  return (
    <div className="side-split">
      {header}
      {problem && <div className="review-error">{problem}</div>}
      <div className="side-split-body gd-panel">
        <ProgressBar p={view.progress} />
        {g && g.context.length > 0 && (
          <Row pick={{ kind: "context" }} picked={picked} onPick={pick}>
            <span className="gd-v" />
            <span className="gd-title">Context</span>
            <span className="gd-n">{g.context.filter((d) => !d.superseded).length}</span>
          </Row>
        )}
        {RISKS.map(riskGroup)}
        {view.commented.length > 0 && (
          <div>
            <GroupHead
              name={
                <span className="gd-group">
                  <span className="gd-dot commented" />
                  Commented
                </span>
              }
              count={view.commented.length}
              shut={isShut("commented")}
              onToggle={() => toggle("commented")}
            />
            {open("commented") &&
              view.commented.map((c) => (
                <Row
                  key={c.id}
                  pick={{ kind: "commented", id: c.id }}
                  picked={picked}
                  onPick={pick}
                >
                  <span className="gd-v" />
                  <span className="gd-title">{threadTitle(c)}</span>
                  <span className="gd-threads">1</span>
                </Row>
              ))}
          </div>
        )}
        {view.tests.length > 0 && (
          <div>
            <GroupHead
              name="Tests"
              count={view.tests.length}
              shut={!shut.has("tests:open")}
              onToggle={() => toggle("tests:open")}
            />
            {shut.has("tests:open") && view.tests.map(partRow)}
          </div>
        )}
        {view.excluded.length > 0 && (
          <div>
            <GroupHead
              name="Excluded"
              count={view.excluded.length}
              shut={!shut.has("excluded:open")}
              onToggle={() => toggle("excluded:open")}
            />
            {shut.has("excluded:open") &&
              reasons.map((reason) => {
                const parts = view.excluded.filter((e) => e.part.reason === reason);
                const key = `excluded:${reason}:open`;
                return (
                  <div key={reason}>
                    <button type="button" className="gd-reason" onClick={() => toggle(key)}>
                      <span className="chevron">{shut.has(key) ? "▾" : "▸"}</span>
                      <span className="gd-title">{REASON_NAMES[reason]}</span>
                      <span className="gd-n">{parts.length}</span>
                    </button>
                    {shut.has(key) && parts.map(partRow)}
                  </div>
                );
              })}
          </div>
        )}
      </div>
      {picker}
    </div>
  );
}
