import type { BotRun } from "../bindings";
import { commands } from "../ipc";
import { IconButton } from "../ui/icons";
import { runState } from "./personas";

export type Act = (f: () => Promise<{ status: string; error?: string }>) => void;

/** Stop while the run is going, or Retry once it halted. */
export function RunControls({ run, act }: { run: BotRun; act: Act }) {
  if (run.active) {
    return (
      <IconButton icon="stop" label="Stop" onClick={() => act(() => commands.stopBots(run.id))} />
    );
  }
  if (runState(run).state !== "halted") return null;
  return (
    <IconButton
      icon="retry"
      label="Retry"
      onClick={() => act(() => commands.continueBots(run.id, run.options))}
    />
  );
}
