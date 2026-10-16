import { useState } from "react";
import type { CurrentReview } from "../review/current";
import { useSendTarget, workspaceTerminals } from "../terminal/registry";
import { Icon } from "../ui/icons";
import { PipeButton } from "../ui/PipeButton";
import { isMac } from "../workbench/keys";
import { loadState, saveState } from "../workbench/savedState";
import type { WorkspaceName } from "../workspaces/context";
import { workspaceRoot } from "../workspaces/useWorkspaces";

const SAVED = "report-prompt";
const DEFAULT = "Review this change. Look for bugs, risky code and missing tests.";

/** What lgtm adds to the prompt, so the agent publishes a report to this review. */
export function reportText(prompt: string, revset: string, root: string | undefined): string {
  const where = root ? ` in ${root}` : "";
  return `${prompt.trim()}

The change is the lgtm review of \`${revset}\`${where}. Read it with lgtm's get_review MCP tool.
When you're done, publish what you found with lgtm's publish_findings MCP tool: one reviewer per pass or persona you used, with your suggested fixes. lgtm shows it as a report.`;
}

/** The review prompt, kept per repo, and the button that pastes it into the send target. */
export function ReportPrompt({
  workspace,
  review,
}: {
  workspace: WorkspaceName;
  review: CurrentReview | undefined;
}) {
  const [prompt, setPrompt] = useState(() =>
    loadState(SAVED, DEFAULT, (v): v is string => typeof v === "string"),
  );
  const { target } = useSendTarget(workspace);
  const title = workspaceTerminals(workspace).find((t) => t.id === target)?.title;
  const ready = !!review && !!prompt.trim();

  function create() {
    if (!review || !prompt.trim()) return;
    review.send(reportText(prompt, review.revset, workspaceRoot(workspace)));
  }

  return (
    <div className="report-prompt">
      <textarea
        id="report-prompt"
        value={prompt}
        rows={5}
        aria-label="Review prompt"
        spellCheck={false}
        onChange={(e) => {
          setPrompt(e.target.value);
          saveState(SAVED, e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (isMac ? e.metaKey : e.ctrlKey)) {
            e.preventDefault();
            create();
          }
        }}
      />
      <div className="report-prompt-bar">
        {target && (
          <span className="report-target">
            <Icon name="terminal" size={11} />
            <b>{title || "Terminal"}</b>
          </span>
        )}
        <span className="spacer" />
        <PipeButton disabled={!ready} onClick={create} keys="⌘↵">
          Create Report
        </PipeButton>
      </div>
    </div>
  );
}
