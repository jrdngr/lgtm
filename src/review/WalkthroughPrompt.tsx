import { useCallback, useEffect, useState } from "react";
import type { PromptFile, Scope } from "../bindings";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import type { MenuItem } from "../ui/Menu";
import { useWorkspace } from "../workspaces/context";

/** The walkthrough's prompt as menu items: which prompt it uses, then edits of the global or
 * repo prompt file, written from the built-in prompt the first time. */
export function useWalkthroughPrompt(): { items: MenuItem[]; error?: string } {
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

  const items: MenuItem[] = [
    {
      label: `Prompt: ${file ? file.scope : "built-in"}`,
      title: file?.path ?? "No walkthrough.md, so the built-in prompt",
      disabled: true,
      onSelect() {},
    },
    {
      label: "Edit global prompt",
      title: "~/.config/lgtm/walkthrough.md",
      onSelect: () => edit("global"),
    },
    {
      label: "Edit repo prompt",
      title: ".lgtm/walkthrough.md, used over the global one",
      onSelect: () => edit("repo"),
    },
  ];
  return { items, error };
}
