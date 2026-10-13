import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { events } from "../bindings";
import { commands } from "../ipc";
import { remembered } from "./remembered";

/** View > Word Wrap: long lines wrap in reviews and editors instead of scrolling sideways. */
export const wordWrap = remembered("lgtm:wordWrap", false, [true, false]);

/** Keeps the menu's check and this window's setting in step. */
export function useWordWrapMenu() {
  useEffect(() => {
    commands.setWordWrap(wordWrap.get());
    const sub = events
      .wordWrap(getCurrentWebviewWindow())
      .listen((e) => wordWrap.set(e.payload.on));
    return () => {
      sub.then((stop) => stop());
    };
  }, []);
}
