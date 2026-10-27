import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useSyncExternalStore } from "react";
import { events } from "../bindings";
import { commands } from "../ipc";
import { activeFrame, api, confirmClose, setIframe, showRepo } from "./store";
import "./shell.css";

/** A project window: each awake repo in a frame of its own, the one on screen shown. Hidden
 * frames run on, as hidden workspaces do, and stay laid out at the window's size: WKWebView
 * gives a `display: none` frame that missed a zoom change the wrong scale when it shows. */
export function Shell() {
  const s = useSyncExternalStore(api.subscribe, api.state);

  useEffect(() => {
    const win = getCurrentWebviewWindow();
    const subs = [
      win.onCloseRequested(async (e) => {
        // Closed by the backend once confirmed, which needs no window permission.
        e.preventDefault();
        if (await confirmClose()) await commands.closeWindow();
        else await commands.keepWindow();
      }),
      events.addRepo(win).listen(() => activeFrame()?.addRepo()),
      events.showRepo(win).listen((e) => showRepo(e.payload.label)),
    ];
    return () => {
      for (const sub of subs) sub.then((stop) => stop());
    };
  }, []);

  return (
    <div className="shell">
      {s.project.repos
        .filter((r) => !r.asleep)
        .map((r) => (
          <iframe
            key={r.label}
            ref={(el) => setIframe(r.label, el)}
            src={`?repo=${encodeURIComponent(r.label)}`}
            title={r.name}
            className={r.label === s.active ? undefined : "away"}
          />
        ))}
    </div>
  );
}
