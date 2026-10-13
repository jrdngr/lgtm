import { Channel } from "@tauri-apps/api/core";
import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useRef } from "react";
import { flushSync } from "react-dom";
import { commands, type PtyEvent } from "../ipc";
import { bottom } from "../workbench/bottom";
import { useWorkspace } from "../workspaces/context";
import { registerTerminal, touchTerminal } from "./registry";
import { createTerminal } from "./xterm";

type Params = { setup?: boolean };

export function TerminalPanel({ api, params }: IDockviewPanelProps<Params>) {
  const ref = useRef<HTMLDivElement>(null);
  const workspace = useWorkspace();
  const setup = useRef(params.setup ?? false);

  useEffect(() => {
    const container = ref.current as HTMLDivElement;
    const { term, refit } = createTerminal(container);
    let id: number | undefined;
    let disposed = false;

    const channel = new Channel<PtyEvent>();
    channel.onmessage = (e) => {
      if (e.kind === "data") term.write(e.data);
      else term.write(`\r\n[process exited with code ${e.code}]\r\n`);
    };

    // Deferred so a mount that's undone at once (React StrictMode) never starts a shell, and
    // setup never starts twice.
    const spawn = window.setTimeout(() => {
      const { cols, rows } = term;
      commands.ptySpawn(workspace, cols, rows, setup.current, channel).then((r) => {
        if (r.status === "error") {
          term.write(`failed to start shell: ${r.error}\r\n`);
        } else if (disposed) {
          commands.ptyKill(r.data);
        } else {
          id = r.data;
          // Resizes while it started weren't sent.
          if (term.cols !== cols || term.rows !== rows)
            commands.ptyResize(id, term.cols, term.rows);
        }
      });
      // Setup runs once, not again when the layout is restored.
      if (setup.current) {
        setup.current = false;
        api.updateParameters({ setup: undefined });
      }
    });

    let title = "";
    const unregister = registerTerminal(api.id, {
      workspace,
      title: () => title,
      canPaste: () => term.modes.bracketedPasteMode,
      paste: (text) => term.paste(text),
      focus: () => {
        // A terminal in the hidden bottom panel shows it first, or it can't take focus.
        if (container.closest(".bottom-panel")) flushSync(() => bottom.set({ open: true }));
        api.setActive();
        term.focus();
      },
    });
    const onFocus = () => touchTerminal(api.id);
    term.textarea?.addEventListener("focus", onFocus);

    const subs = [
      term.onData((data) => id !== undefined && commands.ptyWrite(id, data)),
      term.onTitleChange((t) => {
        title = t;
      }),
      term.onResize(({ cols, rows }) => id !== undefined && commands.ptyResize(id, cols, rows)),
      api.onDidActiveChange(({ isActive }) => isActive && term.focus()),
    ];
    const observer = new ResizeObserver(refit);
    observer.observe(container);
    if (api.isActive) term.focus();

    return () => {
      disposed = true;
      clearTimeout(spawn);
      unregister();
      term.textarea?.removeEventListener("focus", onFocus);
      observer.disconnect();
      for (const s of subs) s.dispose();
      if (id !== undefined) commands.ptyKill(id);
      term.dispose();
    };
  }, [api, workspace]);

  return <div ref={ref} className="terminal" />;
}
