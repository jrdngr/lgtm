import { useCallback, useEffect, useState } from "react";
import type { Prompt, PromptInfo, Scope } from "../bindings";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import { useWorkspace } from "../workspaces/context";
import { Section } from "./SettingsPanel";

const PROMPTS: Record<Prompt, { name: string; help: string }> = {
  walkthrough: { name: "Walkthrough", help: "What the walkthrough agent looks for and writes." },
};

const SCOPES: [Scope, string][] = [
  ["global", "Global"],
  ["repo", "This repo"],
];

/** One prompt's two files: open (creating from the built-in prompt) or remove each. */
function PromptRow({ info, onChange }: { info: PromptInfo; onChange(): void }) {
  const workspace = useWorkspace();
  const [confirm, setConfirm] = useState<Scope>();
  const [error, setError] = useState<string>();
  const { name, help } = PROMPTS[info.prompt];

  async function edit(scope: Scope) {
    const r = await commands.editPrompt(workspace, info.prompt, scope);
    if (r.status === "error") return setError(r.error);
    onChange();
    openFile(workspace, r.data);
  }

  async function remove(scope: Scope) {
    setConfirm(undefined);
    const r = await commands.removePrompt(workspace, info.prompt, scope);
    setError(r.status === "error" ? r.error : undefined);
    onChange();
  }

  return (
    <div className="settings-prompt">
      <div className="settings-prompt-head">
        <b>{name}</b>
        <span className="pill">In use: {info.inUse ?? "built-in"}</span>
      </div>
      <div className="settings-help">{help}</div>
      {SCOPES.map(([scope, label]) => {
        const path = scope === "global" ? info.global : info.repo;
        const exists = scope === "global" ? info.globalExists : info.repoExists;
        return (
          <div key={scope} className="settings-prompt-file">
            <span className="settings-prompt-scope">{label}</span>
            <code className="settings-path" title={path ?? undefined}>
              {path ?? "no home directory"}
            </code>
            {confirm === scope ? (
              <>
                <span className="settings-help">Delete this file?</span>
                <button type="button" className="quiet" onClick={() => setConfirm(undefined)}>
                  Cancel
                </button>
                <button type="button" className="danger" onClick={() => remove(scope)}>
                  Delete
                </button>
              </>
            ) : (
              <>
                <button type="button" onClick={() => edit(scope)} disabled={!path}>
                  {exists ? "Edit" : "Create"}
                </button>
                {exists && (
                  <button type="button" className="quiet" onClick={() => setConfirm(scope)}>
                    Delete…
                  </button>
                )}
              </>
            )}
          </div>
        );
      })}
      {error && <div className="error">{error}</div>}
    </div>
  );
}

/** The agent prompts: the repo's file wins over the global one, which wins over the built-in
 * prompt. Create copies the built-in prompt into the file to start from. */
export function PromptSettings() {
  const workspace = useWorkspace();
  const [infos, setInfos] = useState<PromptInfo[]>();
  const [error, setError] = useState<string>();
  const load = useCallback(async () => {
    const r = await commands.promptFiles(workspace);
    if (r.status === "ok") setInfos(r.data);
    setError(r.status === "error" ? r.error : undefined);
  }, [workspace]);
  useEffect(() => {
    load();
  }, [load]);

  return (
    <Section id="prompts" title="Prompts">
      <p className="settings-help">
        A repo's prompt file is used over the global one, and the global one over the built-in
        prompt. Delete both to go back to the built-in prompt. The format agents reply in is fixed.
      </p>
      {infos?.map((info) => (
        <PromptRow key={info.prompt} info={info} onChange={load} />
      ))}
      {error && <div className="error">{error}</div>}
    </Section>
  );
}
