import { useRef, useState } from "react";
import { dockOf } from "../core/docks";
import { isMac } from "../core/keys";
import { openNote } from "../core/panels";
import { loadState, saveState } from "../core/savedState";
import type { WorkspaceName } from "../core/workspace";
import { commands, errorText, unwrap } from "../ipc";
import { useSendText } from "../review/useSendText";
import { stampName } from "./store";

const isString = (v: unknown): v is string => typeof v === "string";

/** A box for jotting a thought down: Save files it as a workspace note named by the time,
 * Send pastes it into the workspace's agent terminal. Either clears it. The draft is kept per
 * workspace. */
export function QuickNote({ workspace }: { workspace: WorkspaceName }) {
  const key = `quickNote:${workspace ?? ""}`;
  const [text, setText] = useState(() => loadState(key, "", isString));
  const [error, setError] = useState<string>();
  const box = useRef<HTMLTextAreaElement>(null);
  const focus = () => box.current?.focus();
  const { sendText, picker } = useSendText(workspace, setError, focus);

  const update = (next: string) => {
    setText(next);
    saveState(key, next);
  };
  const empty = !text.trim();

  async function save() {
    if (empty) return;
    try {
      const n = await unwrap(
        commands.createNote(workspace, "workspace", "", stampName(new Date()), text),
      );
      update("");
      setError(undefined);
      const api = dockOf(workspace);
      if (api) openNote(api, n.id, n.name, true);
      requestAnimationFrame(focus);
    } catch (e) {
      setError(errorText(e));
    }
  }

  const send = () => {
    if (!empty) sendText(text, () => update(""));
  };

  function onKeyDown(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (!mod || e.key !== "Enter") return;
    e.preventDefault();
    if (e.shiftKey) send();
    else save();
  }

  return (
    <div className="quick-note">
      <textarea
        ref={box}
        aria-label="Quick note"
        value={text}
        onChange={(e) => update(e.target.value)}
        onKeyDown={onKeyDown}
      />
      <div className="quick-note-actions">
        <button type="button" disabled={empty} onClick={send}>
          Send<kbd>⇧⌘↵</kbd>
        </button>
        <button type="button" className="primary" disabled={empty} onClick={save}>
          Save<kbd>⌘↵</kbd>
        </button>
      </div>
      {error && <div className="review-error">{error}</div>}
      {picker}
    </div>
  );
}
