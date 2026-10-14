import type { EditorView } from "@codemirror/view";
import { Channel } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { endFrom, startRun } from "../agents/runs";
import type { SelectedLines } from "../bindings";
import { commands } from "../ipc";
import type { WorkspaceName } from "../workspaces/context";
import { AgentBox, type Box, type Reload, type Turn } from "./AgentBox";
import { placeAgentBox, selectedLines } from "./boxWidget";
import { baseName } from "./open";

let nextKey = 1;

/** The agent box under a selection in an editor tab: one at a time, gone when it closes or
 * the tab does. `reload` reads the file again after a change. */
export function useAgentBox(
  workspace: WorkspaceName,
  path: string,
  view: EditorView | undefined,
  reload: () => Promise<Reload>,
  show: () => void,
) {
  const [box, setBox] = useState<Box>();
  const dom = useRef<HTMLElement>(undefined);
  dom.current ??= document.createElement("div");
  /** The open box's key, to drop replies for a box that's gone. */
  const current = useRef<string>(undefined);
  current.current = box?.key;
  const running = useRef<string>(undefined);

  // The box goes with the editor it sits in, and its run with it.
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs when the editor goes
  useEffect(
    () => () => {
      stopRun();
      setBox(undefined);
    },
    [view],
  );

  function open(change: boolean) {
    if (!view) return;
    const { start, end, text, pos } = selectedLines(view.state);
    stopRun();
    const lines: SelectedLines = { path, start, end, text };
    const where = `${baseName(path)}:${start === end ? start : `${start}-${end}`}`;
    setBox({ key: `${nextKey++}`, change, where, lines, turns: [], session: null });
    view.dispatch({ effects: placeAgentBox.of({ pos, dom: dom.current as HTMLElement }) });
  }

  function stopRun() {
    if (running.current) commands.stopSelectionAsk(running.current);
    running.current = undefined;
  }

  function close() {
    stopRun();
    setBox(undefined);
    view?.dispatch({ effects: placeAgentBox.of(null) });
    view?.focus();
  }

  /** Updates the box under `key`, if it's still open. */
  const update = (key: string, f: (b: Box) => Box) => setBox((b) => (b?.key === key ? f(b) : b));

  async function send(message: string, change: boolean) {
    if (!box || running.current || !message.trim()) return;
    const { key, lines, where, session } = box;
    running.current = key;
    update(key, (b) => ({
      ...b,
      change,
      reload: undefined,
      turns: [...b.turns, { ask: message, change, reply: "" }],
      running: true,
    }));
    const handle = startRun(workspace, {
      kind: change ? "change" : "ask",
      subject: where,
      stop: () => commands.stopSelectionAsk(key),
      open: show,
    });
    const onReply = (f: (reply: string) => string, more: Partial<Turn> = {}) =>
      update(key, (b) => {
        const turns = [...b.turns];
        const last = turns[turns.length - 1];
        turns[turns.length - 1] = { ...last, ...more, reply: f(last.reply) };
        return { ...b, turns };
      });
    const channel = new Channel<string>();
    channel.onmessage = (text) => onReply((r) => r + text);
    const r = await commands.askSelection(
      workspace,
      key,
      { lines, message: message.trim(), change, session },
      channel,
    );
    if (r.status === "ok" && r.data.stopped) handle.end("stopped");
    else endFrom(handle, r);
    if (running.current === key) running.current = undefined;
    if (r.status === "error") onReply(() => r.error, { failed: true });
    else {
      const { text, stopped } = r.data;
      onReply(() => (stopped ? `${text.trimEnd()} (stopped)` : text), {
        changed: change && !stopped,
      });
    }
    const done = r.status === "ok" && !r.data.stopped;
    update(key, (b) => ({
      ...b,
      running: false,
      session: (r.status === "ok" && r.data.session) || b.session,
    }));
    // A stopped change may have edited some files, so the tab reads them either way.
    if (!change || current.current !== key) return;
    const reloaded = await reload();
    if (done) update(key, (b) => ({ ...b, reload: reloaded }));
  }

  const portal =
    box &&
    createPortal(
      <AgentBox
        key={box.key}
        box={box}
        send={send}
        stop={() => commands.stopSelectionAsk(box.key)}
        close={close}
      />,
      dom.current,
    );

  return { open, close, isOpen: !!box, portal };
}
