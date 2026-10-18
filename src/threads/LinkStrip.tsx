import type { WorkspaceName } from "../core/workspace";
import { unlinkAgent, useLink } from "../review/linkedAgent";
import { Icon } from "../ui/icons";

/** The terminal linked to the PR review: its name, whether its agent is working, and Unlink. */
export function LinkStrip({ workspace }: { workspace: WorkspaceName }) {
  const link = useLink(workspace);
  if (!link.terminal) return null;
  return (
    <div className="link-strip">
      <Icon name="terminal" size={13} />
      <span className="link-title">{link.title || "Terminal"}</span>
      <span className={`link-dot${link.working ? " working" : ""}`} />
      <span className="spacer" />
      <button type="button" className="pr-head-btn" onClick={() => unlinkAgent(workspace)}>
        Unlink
      </button>
    </div>
  );
}
