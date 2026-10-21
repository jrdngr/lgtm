import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useState, useSyncExternalStore } from "react";
import { type DrawingDto, events } from "../bindings";
import { commands, errorText, unwrap } from "../ipc";

let changes = 0;
const listeners = new Set<() => void>();
const subscribe = (f: () => void) => {
  listeners.add(f);
  return () => listeners.delete(f);
};

/** Has every drawing list and open drawing load again. */
export function drawingsChanged() {
  changes++;
  for (const f of listeners) f();
}

/** Loads again whenever a drawing is made, renamed, deleted or drawn on by an agent. */
function useVersion() {
  const version = useSyncExternalStore(subscribe, () => changes);
  useEffect(() => {
    const sub = events.drawingChanged(getCurrentWebviewWindow()).listen(drawingsChanged);
    return () => {
      sub.then((un) => un());
    };
  }, []);
  return version;
}

/** The repo's drawings, the last changed first. */
export function useDrawings() {
  const [items, setItems] = useState<DrawingDto[]>();
  const [error, setError] = useState<string>();
  const version = useVersion();
  // biome-ignore lint/correctness/useExhaustiveDependencies: `version` asks for a reload
  useEffect(() => {
    let live = true;
    unwrap(commands.drawings()).then(
      (d) => {
        if (!live) return;
        setItems(d);
        setError(undefined);
      },
      (e) => live && setError(errorText(e)),
    );
    return () => {
      live = false;
    };
  }, [version]);
  return { items, error };
}

/** Drawing `id` as stored, loaded again when an agent draws on it. */
export function useDrawing(id: string) {
  const [drawing, setDrawing] = useState<DrawingDto>();
  const [error, setError] = useState<string>();
  const version = useVersion();
  // biome-ignore lint/correctness/useExhaustiveDependencies: `version` asks for a reload
  useEffect(() => {
    let live = true;
    unwrap(commands.drawing(id)).then(
      (d) => {
        if (!live) return;
        setDrawing(d);
        setError(undefined);
      },
      (e) => live && setError(errorText(e)),
    );
    return () => {
      live = false;
    };
  }, [id, version]);
  return { drawing, error };
}

let pendingNew = false;
const newListeners = new Set<() => void>();

/** Asks the Drawings panel to start naming a new drawing, now or once it shows. */
export function requestNewDrawing() {
  if (newListeners.size) for (const f of newListeners) f();
  else pendingNew = true;
}

export function onNewDrawing(f: () => void) {
  newListeners.add(f);
  if (pendingNew) {
    pendingNew = false;
    f();
  }
  return () => {
    newListeners.delete(f);
  };
}
