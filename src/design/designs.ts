import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useCallback, useEffect, useState, useSyncExternalStore } from "react";
import { type DesignDto, type DesignItemDto, events } from "../bindings";
import { type WorkspaceName, workspaceKey } from "../core/workspace";
import { commands, errorText, unwrap } from "../ipc";
import { keepSubmit, submitNextSend } from "../terminal/registry";

let changes = 0;
const listeners = new Set<() => void>();
const subscribe = (f: () => void) => {
  listeners.add(f);
  return () => listeners.delete(f);
};
const bump = () => {
  changes++;
  for (const f of listeners) f();
};

/** Has every list of designs load again. */
export const designsChanged = bump;

/** The repo's designs with their states, loaded again when one is saved, deleted or written. */
export function useDesigns(workspace: WorkspaceName) {
  const [items, setItems] = useState<DesignItemDto[]>();
  const [error, setError] = useState<string>();
  const version = useSyncExternalStore(subscribe, () => changes);

  // biome-ignore lint/correctness/useExhaustiveDependencies: `version` asks for a reload
  useEffect(() => {
    let live = true;
    unwrap(commands.designs(workspace)).then(
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
  }, [workspace, version]);

  useEffect(() => {
    const sub = events.designChanged(getCurrentWebviewWindow()).listen(bump);
    return () => {
      sub.then((un) => un());
    };
  }, []);

  return { items, error, reload: useCallback(bump, []) };
}

/** What the Architecture tab maps: the code, or a design over it or alone. */
export interface Source {
  design?: string;
  overCode: boolean;
}

let source: Source = { overCode: true };
const sourceListeners = new Set<() => void>();
const subscribeSource = (f: () => void) => {
  sourceListeners.add(f);
  return () => sourceListeners.delete(f);
};

export function setSource(next: Partial<Source>) {
  source = { ...source, ...next };
  for (const f of sourceListeners) f();
}

/** Shows design `id` in the Architecture tab. */
export const showDesign = (id: string) => setSource({ design: id });
export const showCode = () => setSource({ design: undefined });

export const useSource = () => useSyncExternalStore(subscribeSource, () => source);

/** The prompt asking a terminal agent to write design `id`'s map, sent with Enter. */
export async function designText(workspace: WorkspaceName, id: string): Promise<string> {
  submitNextSend();
  return unwrap(keepSubmit(commands.designPrompt(workspace, id)));
}

/** The design form open in a window: a new design, or one to edit. */
interface Editing {
  workspace: string;
  design?: DesignDto;
}

let editing: Editing | undefined;
const editListeners = new Set<() => void>();

function setEditing(next: Editing | undefined) {
  editing = next;
  for (const f of editListeners) f();
}

/** Opens the design form on `design`, or on a new one. */
export const editDesign = (workspace: WorkspaceName, design?: DesignDto) =>
  setEditing({ workspace: workspaceKey(workspace), design });
export const closeDesignForm = () => setEditing(undefined);

export function useEditing(workspace: WorkspaceName) {
  const e = useSyncExternalStore(
    (f) => {
      editListeners.add(f);
      return () => editListeners.delete(f);
    },
    () => editing,
  );
  return e?.workspace === workspaceKey(workspace) ? e : undefined;
}
