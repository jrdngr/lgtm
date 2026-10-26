import { useState } from "react";
import type { WorkspaceName } from "../core/workspace";
import { errorText } from "../ipc";
import { useSendText } from "../review/useSendText";
import { designText } from "./designs";

/** Sends a design's prompt to a terminal agent. `busy` is the design whose prompt is being
 * put together; `picker` is the terminal picker to render. */
export function useDesignGenerate(
  workspace: WorkspaceName,
  setError: (e: string | undefined) => void,
) {
  const [busy, setBusy] = useState<string>();
  const { sendText, picker } = useSendText(workspace, setError, () => {}, "background");
  const generate = async (id: string) => {
    setBusy(id);
    try {
      sendText(await designText(workspace, id));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(undefined);
    }
  };
  return { generate, busy, picker };
}
