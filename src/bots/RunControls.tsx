import type { ReactNode } from "react";
import type { BotRun } from "../bindings";
import { commands } from "../ipc";
import { runState } from "./personas";

export type Act = (f: () => Promise<{ status: string; error?: string }>) => void;

/** Stop while the run is going. Otherwise Retry when it halted, then `children`. */
export function RunControls({
  run,
  act,
  children,
}: {
  run: BotRun;
  act: Act;
  children: ReactNode;
}) {
  if (run.active) {
    return (
      <button type="button" onClick={() => act(() => commands.stopBots(run.id))}>
        Stop
      </button>
    );
  }
  return (
    <>
      {runState(run).state === "halted" && (
        <button type="button" onClick={() => act(() => commands.continueBots(run.id, run.options))}>
          Retry
        </button>
      )}
      {children}
    </>
  );
}
