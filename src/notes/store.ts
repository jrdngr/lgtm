import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useState, useSyncExternalStore } from "react";
import { events, type NoteDto, type NotesDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { commands, errorText, unwrap } from "../ipc";

let version = 0;
/** The note the last change named, or undefined when it wasn't one note. */
let changedId: string | undefined;
const listeners = new Set<() => void>();
let listening = false;

/** Listens once for note changes, from any window or an agent. */
function listen() {
  if (listening) return;
  listening = true;
  events.notesChanged(getCurrentWebviewWindow()).listen((e) => {
    version++;
    changedId = e.payload.id ?? undefined;
    for (const f of listeners) f();
  });
}

function useVersion() {
  listen();
  return useSyncExternalStore(
    (f) => {
      listeners.add(f);
      return () => listeners.delete(f);
    },
    () => version,
  );
}

/** Every note and folder the workspace shows, loaded again after any change. */
export function useNotes(workspace: WorkspaceName) {
  const [data, setData] = useState<NotesDto>();
  const [error, setError] = useState<string>();
  const v = useVersion();
  // biome-ignore lint/correctness/useExhaustiveDependencies: `v` asks for a reload
  useEffect(() => {
    let live = true;
    unwrap(commands.notes(workspace)).then(
      (d) => {
        if (!live) return;
        setData(d);
        setError(undefined);
      },
      (e) => live && setError(errorText(e)),
    );
    return () => {
      live = false;
    };
  }, [workspace, v]);
  return { data, error };
}

/** Note `id`, loaded again when it changes. */
export function useNote(workspace: WorkspaceName, id: string) {
  const [note, setNote] = useState<NoteDto>();
  const [error, setError] = useState<string>();
  const v = useVersion();
  const mine = changedId === undefined || changedId === id ? v : -1;
  // biome-ignore lint/correctness/useExhaustiveDependencies: `mine` asks for a reload
  useEffect(() => {
    let live = true;
    unwrap(commands.note(workspace, id)).then(
      (n) => {
        if (!live) return;
        setNote(n);
        setError(undefined);
      },
      (e) => live && setError(errorText(e)),
    );
    return () => {
      live = false;
    };
  }, [workspace, id, mine]);
  return { note, error };
}

const pad = (n: number) => String(n).padStart(2, "0");

/** A quick note's name: when it was saved, like `2026-10-21 19:52`. */
export const stampName = (d: Date) =>
  `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
