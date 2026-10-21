import { useCallback, useEffect, useRef, useState } from "react";
import type { Canvas as Stored } from "../bindings";
import { errorText } from "../ipc";
import { type Canvas, fullCanvas } from "./view";

/** Where a canvas is stored: a drawing, or a design's drawing over its map. */
export interface CanvasStore {
  /** The stored canvas, as it last loaded. */
  stored: Stored | undefined;
  save(c: Canvas): Promise<unknown>;
  /** Puts the canvas back as it was before the last agent pass, returning it. */
  undoAgent(): Promise<Stored | undefined>;
}

/** A canvas as the user edits it: saved on each change, with undo and redo of the user's own
 * changes and of the last agent pass. Takes the stored canvas again whenever it changes
 * underneath, as when an agent draws. */
export function useCanvas(store: CanvasStore) {
  const [canvas, setCanvas] = useState(() => fullCanvas(store.stored));
  const [error, setError] = useState<string>();
  const current = useRef(canvas);
  current.current = canvas;
  const past = useRef<Canvas[]>([]);
  const future = useRef<Canvas[]>([]);
  const seen = useRef(JSON.stringify(store.stored ?? null));
  const saveRef = useRef(store.save);
  saveRef.current = store.save;

  useEffect(() => {
    const now = JSON.stringify(store.stored ?? null);
    if (now === seen.current) return;
    seen.current = now;
    setCanvas(fullCanvas(store.stored));
  }, [store.stored]);

  const save = useCallback((c: Canvas) => {
    setCanvas(c);
    saveRef.current(c).then(
      () => setError(undefined),
      (e) => setError(errorText(e)),
    );
  }, []);

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
    store.undoAgent().then(
      (c) => {
        seen.current = JSON.stringify(c ?? null);
        setCanvas(fullCanvas(c));
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
