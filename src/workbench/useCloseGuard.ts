import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { events } from "../bindings";
import { confirmAll } from "../editor/unsaved";
import { commands } from "../ipc";

/** Closing the window, or swapping it for the start window, asks about unsaved edits first. */
export function useCloseGuard() {
  useEffect(() => {
    const win = getCurrentWebviewWindow();
    const subs = [
      win.onCloseRequested(async (e) => {
        // Closed by the backend once confirmed, which needs no window permission.
        e.preventDefault();
        if (await confirmAll()) await commands.closeWindow();
      }),
      events.switchRepo(win).listen(async () => {
        if (await confirmAll()) await commands.switchRepo();
      }),
    ];
    return () => {
      for (const sub of subs) sub.then((stop) => stop());
    };
  }, []);
}
