import { useCallback, useEffect, useRef, useState } from "react";
import type { DesignDto } from "../../bindings";
import { commands, errorText, unwrap } from "../../ipc";
import { type Canvas, fullCanvas } from "./view";

/** A design's canvas as the user edits it: saved on each change, with undo and redo of the
 * user's own changes and of the last agent pass. Takes the stored canvas again whenever it
 * changes underneath, as when an agent draws. */
export function useCanvas(design: DesignDto) {
  const { id } = design;
  const [canvas, setCanvas] = useState(() => fullCanvas(design.canvas));
  const [error, setError] = useState<string>();
  const current = useRef(canvas);
  current.current = canvas;
  const past = useRef<Canvas[]>([]);
  const future = useRef<Canvas[]>([]);
  const seen = useRef(JSON.stringify(design.canvas ?? null));

  useEffect(() => {
    const now = JSON.stringify(design.canvas ?? null);
    if (now === seen.current) return;
    seen.current = now;
    setCanvas(fullCanvas(design.canvas));
  }, [design.canvas]);

  const save = useCallback(
    (c: Canvas) => {
      setCanvas(c);
      unwrap(commands.saveDesignCanvas(id, c)).then(
        () => setError(undefined),
        (e) => setError(errorText(e)),
      );
    },
    [id],
  );

  /** A change of the user's: one undo step, and the end of the last agent pass's undo. */
  const commit = useCallback(
    (next: Canvas) => {
      past.current.push(current.current);
      future.current = [];
      save({ ...next, agent: null });
    },
    [save],
  );

  const step = (from: Canvas[], to: Canvas[]) => {
    const c = from.pop();
    if (!c) return;
    to.push(current.current);
    save({ ...c, agent: null });
  };

  const undoAgent = () =>
    unwrap(commands.undoDesignAgent(id)).then(
      (d) => {
        seen.current = JSON.stringify(d.canvas ?? null);
        setCanvas(fullCanvas(d.canvas));
      },
      (e) => setError(errorText(e)),
    );

  return {
    canvas,
    error,
    commit,
    undo: () => step(past.current, future.current),
    redo: () => step(future.current, past.current),
    undoAgent,
  };
}
