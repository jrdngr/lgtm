import { useCallback, useEffect, useRef } from "react";
import type { CanvasKind, FocusPart } from "../bindings";
import { commands } from "../ipc";

/** Tells agents which canvas the workspace shows and what's selected on it: on each change,
 * and through the returned callback whenever the user turns back to it. */
export function useCanvasFocus(
  workspace: string | null,
  focus: { kind: CanvasKind; id: string; view: string; selection: string[]; parts: FocusPart[] },
) {
  const last = useRef(focus);
  last.current = focus;
  const send = useCallback(() => {
    commands.setCanvasFocus(workspace, last.current).catch(() => {});
  }, [workspace]);
  const key = JSON.stringify(focus);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `key` stands for the focus
  useEffect(send, [key, send]);
  return send;
}
