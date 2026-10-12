import { useCallback, useEffect, useState } from "react";
import type { PromptFile, Scope } from "../bindings";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import { useWorkspace } from "../workspaces/context";

/** Which prompt the walkthrough uses, with buttons that open the global or repo prompt file,
 * written from the built-in prompt the first time. */
export function WalkthroughPrompt() {
  const workspace = useWorkspace();
  const [file, setFile] = useState<PromptFile | null>(null);
  const [error, setError] = useState<string>();
  const reload = useCallback(async () => {
    const r = await commands.walkthroughPrompt(workspace);
    if (r.status === "ok") setFile(r.data);
    setError(r.status === "error" ? r.error : undefined);
  }, [workspace]);
  useEffect(() => {
    reload();
  }, [reload]);

  async function edit(scope: Scope) {
    const r = await commands.editWalkthroughPrompt(workspace, scope);
    if (r.status === "error") return setError(r.error);
    reload();
    openFile(workspace, r.data);
  }

  return (
    <div className="note-tools">
      <span title={file?.path ?? "No walkthrough.md, so the built-in prompt"}>
        Prompt: {file ? file.scope : "built-in"}
      </span>
      <span className="spacer" />
      <button type="button" onClick={() => edit("global")} title="~/.config/lgtm/walkthrough.md">
        Edit global
      </button>
      <button
        type="button"
        onClick={() => edit("repo")}
        title=".lgtm/walkthrough.md, used over the global one"
      >
        Edit repo
      </button>
      {error && <span className="error">{error}</span>}
    </div>
  );
}
