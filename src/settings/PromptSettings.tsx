import { useCallback, useEffect, useState } from "react";
import type { Prompt, PromptInfo } from "../bindings";
import { useWorkspace } from "../core/workspace";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import { Section } from "./SettingsPanel";

const PROMPTS: Record<Prompt, { name: string; help: string }> = {
  walkthrough: { name: "Walkthrough", help: "What the walkthrough agent looks for and writes." },
  reply: {
    name: "Replies",
    help: "How the agent takes a position on PR threads and drafts replies in your voice.",
  },
  qq: { name: "Q&A questions", help: "How the agent turns a Q&A prompt into questions." },
};

/** One prompt's file: open it (creating it from the built-in prompt) or remove it. */
function PromptRow({ info, onChange }: { info: PromptInfo; onChange(): void }) {
  const workspace = useWorkspace();
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<string>();
  const { name, help } = PROMPTS[info.prompt];

  async function edit() {
    const r = await commands.editPrompt(info.prompt);
    if (r.status === "error") return setError(r.error);
    onChange();
    openFile(workspace, r.data);
  }

  async function remove() {
    setConfirm(false);
    const r = await commands.removePrompt(info.prompt);
    setError(r.status === "error" ? r.error : undefined);
    onChange();
  }

  return (
    <div className="settings-prompt">
      <div className="settings-prompt-head">
        <b>{name}</b>
        <span className="pill">In use: {info.exists ? "file" : "built-in"}</span>
      </div>
      <div className="settings-help">{help}</div>
      <div className="settings-prompt-file">
        <code className="settings-path" title={info.path ?? undefined}>
          {info.path ?? "no home directory"}
        </code>
        {confirm ? (
          <>
            <span className="settings-help">Delete this file?</span>
            <button type="button" className="quiet" onClick={() => setConfirm(false)}>
              Cancel
            </button>
            <button type="button" className="danger" onClick={remove}>
              Delete
            </button>
          </>
        ) : (
          <>
            <button type="button" onClick={edit} disabled={!info.path}>
              {info.exists ? "Edit" : "Create"}
            </button>
            {info.exists && (
              <button type="button" className="quiet" onClick={() => setConfirm(true)}>
                Delete…
              </button>
            )}
          </>
        )}
      </div>
      {error && <div className="error">{error}</div>}
    </div>
  );
}

/** The agent prompts: a file in ~/.config/lgtm wins over the built-in prompt. Create copies the
 * built-in prompt into the file to start from. */
export function PromptSettings() {
  const [infos, setInfos] = useState<PromptInfo[]>();
  const load = useCallback(async () => {
    setInfos(await commands.promptFiles());
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  return (
    <Section id="prompts" title="Prompts">
      <p className="settings-help">
        A prompt file is used over the built-in prompt. Delete it to go back to the built-in prompt.
        The format agents reply in is fixed.
      </p>
      {infos?.map((info) => (
        <PromptRow key={info.prompt} info={info} onChange={load} />
      ))}
    </Section>
  );
}
