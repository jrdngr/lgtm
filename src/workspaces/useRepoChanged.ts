import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useRef } from "react";
import { events } from "../bindings";

/** Calls the latest `cb` whenever this window's repo changes. */
export function useRepoChanged(cb: () => void) {
  const ref = useRef(cb);
  ref.current = cb;
  useEffect(() => {
    // Scoped to this window: other windows hold other repos.
    const sub = events.repoChanged(getCurrentWebviewWindow()).listen(() => ref.current());
    return () => {
      sub.then((un) => un());
    };
  }, []);
}
