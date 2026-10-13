import { useEffect, useRef, useState } from "react";
import { commands } from "../ipc";
import { isMac } from "../workbench/keys";
import type { WorkspaceName } from "../workspaces/context";
import type { Design } from "./model";
import { act, type Busy, runStep, showDesign } from "./store";

interface Props {
  ws: WorkspaceName;
  d: Design;
  busy?: Busy;
  onSend(text: string): void;
}

/** The terminal prompt for drafting in an agent the user drives. */
const terminalText = (d: Design) =>
  `Draft the design "${d.name}" from its brief in ${d.dir}/brief.md. Write it to ${d.dir}/design.md. ` +
  `Then ask me its open questions with lgtm's design_ask tool (design "${d.slug}"), and wait for my answers with design_answers.`;

/** What the design is for, and files the agent should read, then drafting it. */
export function BriefView({ ws, d, busy, onSend }: Props) {
  const [brief, setBrief] = useState(d.brief);
  const [attach, setAttach] = useState(d.attach);
  const [path, setPath] = useState("");
  const ref = useRef<HTMLTextAreaElement>(null);
  const dirty = brief !== d.brief || attach.join("\n") !== d.attach.join("\n");

  // Edits from the agent or the file show unless there are unsaved ones here.
  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;
  useEffect(() => {
    if (dirtyRef.current) return;
    setBrief(d.brief);
    setAttach(d.attach);
  }, [d.brief, d.attach]);

  useEffect(() => ref.current?.focus(), []);

  async function save(next = attach) {
    if (brief === d.brief && next.join("\n") === d.attach.join("\n")) return true;
    const r = await act(ws, commands.saveDesignBrief(ws, d.slug, brief, next));
    return r.status === "ok";
  }

  async function draft() {
    if (busy || !brief.trim() || !(await save())) return;
    runStep(
      ws,
      d.slug,
      {
        kind: "draft",
        subject: `${d.name}: draft`,
        open: () => showDesign(ws, d.slug, "questions"),
      },
      (ch) => commands.draftDesign(ws, d.slug, ch),
    );
    showDesign(ws, d.slug, "questions");
  }

  function addPath() {
    const p = path.trim();
    if (!p || attach.includes(p)) return;
    const next = [...attach, p];
    setAttach(next);
    setPath("");
    save(next);
  }

  function onKey(e: React.KeyboardEvent) {
    if ((isMac ? e.metaKey : e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      draft();
    }
  }

  return (
    <div className="design-brief" onKeyDown={onKey}>
      <label className="qq-label" htmlFor="design-brief">
        Brief
      </label>
      <textarea
        id="design-brief"
        ref={ref}
        className="design-brief-text"
        value={brief}
        placeholder={
          "What to design: the problem, requirements, constraints, what's out of scope.\nA first line like “# Name” names the design."
        }
        onChange={(e) => setBrief(e.target.value)}
        onBlur={() => save()}
      />
      <div className="qq-label">Read first</div>
      <div className="design-attach">
        {attach.map((a) => (
          <span key={a} className="design-chip">
            {a}
            <button
              type="button"
              className="quiet"
              aria-label={`Remove ${a}`}
              onClick={() => {
                const next = attach.filter((x) => x !== a);
                setAttach(next);
                save(next);
              }}
            >
              ×
            </button>
          </span>
        ))}
        <input
          value={path}
          placeholder="A file or folder in the repo, then ↵"
          onChange={(e) => setPath(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              addPath();
            }
          }}
        />
      </div>
      <div className="qq-actions">
        <button
          type="button"
          className="primary"
          disabled={!!busy || !brief.trim()}
          onClick={draft}
        >
          {d.hasDesign ? "Draft again" : "Draft design"} <kbd>⌘↵</kbd>
        </button>
        <button
          type="button"
          disabled={!brief.trim()}
          onClick={async () => (await save()) && onSend(terminalText(d))}
          title="Paste a prompt into a terminal running Claude Code, which asks you through lgtm's MCP tools"
        >
          Send to terminal
        </button>
        {d.hasDesign && (
          <span className="faint">Drafting again replaces the design and its questions</span>
        )}
      </div>
    </div>
  );
}
