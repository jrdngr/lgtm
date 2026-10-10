import { Channel } from "@tauri-apps/api/core";
import type { IDockviewPanelProps } from "dockview-react";
import { useEffect, useRef } from "react";
import { commands, type PtyEvent } from "../ipc";
import { createTerminal } from "./xterm";

export function TerminalPanel({ api }: IDockviewPanelProps) {
  const ref = useRef<HTMLDivElement>(null);

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

    commands.ptySpawn(term.cols, term.rows, null, channel).then((r) => {
      if (r.status === "error") {
        term.write(`failed to start shell: ${r.error}\r\n`);
      } else if (disposed) {
        commands.ptyKill(r.data);
      } else {
        id = r.data;
      }
    });

    const subs = [
      term.onData((data) => id !== undefined && commands.ptyWrite(id, data)),
      term.onResize(({ cols, rows }) => id !== undefined && commands.ptyResize(id, cols, rows)),
      api.onDidActiveChange(({ isActive }) => isActive && term.focus()),
    ];
    const observer = new ResizeObserver(refit);
    observer.observe(container);
    if (api.isActive) term.focus();

    return () => {
      disposed = true;
      observer.disconnect();
      for (const s of subs) s.dispose();
      if (id !== undefined) commands.ptyKill(id);
      term.dispose();
    };
  }, [api]);

  return <div ref={ref} className="terminal" />;
}
