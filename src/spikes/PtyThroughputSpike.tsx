import { Channel } from "@tauri-apps/api/core";
import type { Terminal } from "@xterm/xterm";
import { useEffect, useRef, useState } from "react";
import { commands, type PtyEvent, unwrap } from "../ipc";
import { createTerminal, type Renderer } from "../terminal/xterm";

// 32 MiB of random bytes, base64-encoded: about 43 MiB of terminal output.
const COMMAND = ["sh", "-c", "head -c 33554432 /dev/urandom | base64"];

type Result = { mib: number; ipcMs: number; renderMs: number };

const rate = (mib: number, ms: number) => `${(mib / (ms / 1000)).toFixed(1)} MiB/s`;

export function PtyThroughputSpike() {
  const ref = useRef<HTMLDivElement>(null);
  const term = useRef<{ term: Terminal; renderer: Renderer; refit: () => void }>(null);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<Result>();
  const [error, setError] = useState<string>();

  useEffect(() => {
    const t = createTerminal(ref.current as HTMLDivElement);
    term.current = t;
    const observer = new ResizeObserver(t.refit);
    observer.observe(ref.current as HTMLDivElement);
    return () => {
      observer.disconnect();
      t.term.dispose();
    };
  }, []);

  function run() {
    const t = term.current;
    if (!t || running) return;
    t.term.reset();
    setRunning(true);
    setResult(undefined);
    setError(undefined);

    const start = performance.now();
    let chars = 0;
    const channel = new Channel<PtyEvent>();
    channel.onmessage = (e) => {
      if (e.kind === "data") {
        chars += e.data.length;
        t.term.write(e.data);
        return;
      }
      const ipcMs = performance.now() - start;
      t.term.write("", () => {
        setResult({ mib: chars / 2 ** 20, ipcMs, renderMs: performance.now() - start });
        setRunning(false);
      });
    };
    unwrap(commands.ptySpawn(t.term.cols, t.term.rows, COMMAND, channel)).catch((e) => {
      setError(String(e));
      setRunning(false);
    });
  }

  return (
    <div className="spike">
      <div className="spike-bar">
        <button type="button" onClick={run} disabled={running}>
          {running ? "Running" : "Run"}
        </button>
        <span>renderer: {term.current?.renderer ?? "?"}</span>
        {result && (
          <span>
            {result.mib.toFixed(1)} MiB · all output received in {result.ipcMs.toFixed(0)} ms (
            {rate(result.mib, result.ipcMs)}) · rendered in {result.renderMs.toFixed(0)} ms (
            {rate(result.mib, result.renderMs)})
          </span>
        )}
        {error && <span className="error">{error}</span>}
      </div>
      <div ref={ref} className="terminal" />
    </div>
  );
}
