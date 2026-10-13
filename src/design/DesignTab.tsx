import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useRef, useState } from "react";
import { commands } from "../ipc";
import { useSendText } from "../review/useSendText";
import { openSettings } from "../settings/open";
import { useAgentsOff } from "../tools/agents";
import { Menu } from "../ui/Menu";
import { useWorkspace } from "../workspaces/context";
import { BriefView } from "./BriefView";
import { DocsView } from "./DocsView";
import { FilesView } from "./FilesView";
import {
  type Design,
  decisionsText,
  lastDecided,
  openQuestions,
  qqText,
  STEP_NAMES,
  STEP_VIEW,
  STEPS,
  step,
  type View,
} from "./model";
import { NewDesign } from "./NewDesign";
import { QuestionsView } from "./QuestionsView";
import { busyName, selectDesign, setError, setView, useDesigns } from "./store";
import "./design.css";

const VIEWS: [View, string][] = [
  ["brief", "Brief"],
  ["questions", "Questions"],
  ["docs", "Docs"],
  ["files", "Files"],
];

/** Where the design is in its loop, each step marked done, current or to come. */
function Stepper({ d }: { d: Design }) {
  const at = STEPS.indexOf(step(d));
  return (
    <ol className="design-steps">
      {STEPS.map((s, i) => (
        <li key={s} className={i < at ? "done" : i === at ? "current" : undefined}>
          {i < at ? "✓ " : ""}
          {STEP_NAMES[s]}
        </li>
      ))}
    </ol>
  );
}

/** The Design tab: one design at a time, from its brief through questions to docs. */
export function DesignTab({ api }: IDockviewPanelProps) {
  const ws = useWorkspace();
  const state = useDesigns(ws);
  const agentsOff = useAgentsOff();
  const [creating, setCreating] = useState(false);
  const [focusKey, setFocusKey] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const { sendText, picker } = useSendText(
    ws,
    (e) => setError(ws, e),
    () => rootRef.current?.focus(),
  );
  const d = state.designs.find((x) => x.slug === state.current);
  const busy = d ? state.busy[d.slug] : undefined;
  const view: View = state.view ?? (d ? STEP_VIEW[step(d)] : "brief");
  const doc = state.doc && d?.docs.includes(state.doc) ? state.doc : (d?.docs[0] ?? "design.md");

  // Keys go to the view as soon as the tab is picked.
  useEffect(() => {
    if (api.isActive) setFocusKey((n) => n + 1);
    const sub = api.onDidActiveChange(({ isActive }) => isActive && setFocusKey((n) => n + 1));
    return () => sub.dispose();
  }, [api]);

  if (!state.loaded) return null;
  if (!d || creating)
    return (
      <div className="design design-empty">
        <p>
          A design starts from a brief. The agent drafts it, asks its open questions one at a time,
          and revises; then it writes docs, diagrams and HTML pages.
        </p>
        <NewDesign ws={ws} onDone={d ? () => setCreating(false) : undefined} />
        {state.error && <div className="review-error">{state.error}</div>}
      </div>
    );

  const q = openQuestions(d)[0];
  const prev = lastDecided(d);
  return (
    <div ref={rootRef} className="design" tabIndex={-1}>
      <header className="design-head">
        <select
          className="design-switch"
          value={d.slug}
          aria-label="Design"
          onChange={(e) =>
            e.target.value === "" ? setCreating(true) : selectDesign(ws, e.target.value)
          }
        >
          {state.designs.map((x) => (
            <option key={x.slug} value={x.slug}>
              {x.name}
            </option>
          ))}
          <option value="">New design…</option>
        </select>
        <Stepper d={d} />
        <span className="spacer" />
        <nav className="design-views">
          {VIEWS.map(([v, label]) => (
            <button
              key={v}
              type="button"
              className={v === view ? "current" : "quiet"}
              onClick={() => setView(ws, v)}
            >
              {label}
              {v === "questions" && openQuestions(d).length > 0 && (
                <span className="pill amber">{openQuestions(d).length}</span>
              )}
            </button>
          ))}
        </nav>
        <Menu
          items={[
            q && {
              label: "Copy question as qq text",
              onSelect: () => navigator.clipboard.writeText(qqText(q, prev)),
            },
            {
              label: "Copy decisions",
              disabled: !prev,
              onSelect: () => navigator.clipboard.writeText(decisionsText(d)),
            },
            {
              label: "Edit design prompts…",
              title: "How the agent drafts, asks, and writes docs",
              onSelect: () => openSettings(ws, "prompts"),
            },
          ]}
        />
      </header>
      {busy && (
        <div className="design-busy">
          <span className="design-spin" />
          <span>
            {busyName(busy)}
            {busy.activity && ` · ${busy.activity}`}
          </span>
          <span className="spacer" />
          <button type="button" onClick={() => commands.stopDesign(ws, d.slug)}>
            Stop
          </button>
        </div>
      )}
      {state.error && <div className="review-error">{state.error}</div>}
      {agentsOff && (
        <div className="review-error">Claude Code isn't set up, so agents can't run</div>
      )}
      <div className="design-body">
        {view === "brief" && <BriefView ws={ws} d={d} busy={busy} onSend={sendText} />}
        {view === "questions" && (
          <QuestionsView ws={ws} d={d} busy={busy} streams={state.streams} focusKey={focusKey} />
        )}
        {view === "docs" &&
          (d.docs.length ? (
            <DocsView ws={ws} d={d} doc={doc} busy={busy} streams={state.streams} />
          ) : (
            <div className="placeholder">No docs yet. Draft the design first.</div>
          ))}
        {view === "files" && <FilesView ws={ws} d={d} busy={busy} />}
      </div>
      {picker}
    </div>
  );
}
