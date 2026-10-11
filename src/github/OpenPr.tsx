import { useState } from "react";
import { commands } from "../ipc";

interface Props {
  /** Called with the PR's workspace, and whether it's new and has a setup script. */
  onOpened(workspace: string, setup: boolean): void;
  onClose(): void;
  onError(message: string): void;
}

/** Takes a PR number or URL and checks the PR out into its own workspace. */
export function OpenPr({ onOpened, onClose, onError }: Props) {
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);

  async function open(e: React.FormEvent) {
    e.preventDefault();
    if (!input.trim()) return;
    setBusy(true);
    const r = await commands.openPr(input.trim());
    setBusy(false);
    if (r.status === "error") return onError(r.error);
    setInput("");
    onOpened(r.data.workspace, r.data.setup);
  }

  return (
    <form className="workspace-form" onSubmit={open}>
      <input
        // biome-ignore lint/a11y/noAutofocus: the form opens to take a PR
        autoFocus
        value={input}
        placeholder="PR number or URL"
        spellCheck={false}
        aria-label="Pull request"
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      />
      <button type="submit" disabled={busy || !input.trim()}>
        {busy ? "Fetching" : "Review"}
      </button>
    </form>
  );
}
