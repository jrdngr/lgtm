import { convertFileSrc } from "@tauri-apps/api/core";
import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import { isMac } from "../core/keys";
import type { WorkspaceName } from "../core/workspace";
import { commands, errorText, unwrap } from "../ipc";
import { openUrl } from "../services/pathMenu";
import { IconButton } from "../ui/icons";
import { confirmAction } from "../ui/prompt";
import { useRepoChanged } from "../workspaces/useRepoChanged";
import { appKey, fitted, pagePath, pageUrl, stepZoom, type Trail, visit, webUrl } from "./htmlNav";
import type { EditorParams } from "./open";
import { ZoomControls, zoomKey } from "./ZoomControls";

const base = () => convertFileSrc("", "lgtmhtml");

/** Posts to the script in the frame's page. */
function send(frame: HTMLIFrameElement | null, message: object) {
  frame?.contentWindow?.postMessage({ lgtm: true, ...message }, "*");
}

interface Props {
  workspace: WorkspaceName;
  path: string;
  /** The Rendered/Editor switch, at the end of the bar. */
  toggle: React.ReactNode;
  /** How the tab was left before a restart. */
  saved: Pick<EditorParams, "page" | "pageScroll" | "zoom">;
  /** Keeps how the tab is now, for after a restart. */
  save(values: Pick<EditorParams, "page" | "pageScroll" | "zoom">): void;
}

/** A message from the script the scheme puts first in every page. */
type FrameMessage =
  | { type: "nav"; url: string }
  | { type: "open"; url: string }
  | { type: "zoom"; zoom: number }
  | { type: "found"; found: boolean }
  | { type: "scroll"; x: number; y: number }
  | { type: "scrolled"; x: number; y: number }
  | ({ type: "key" } & Record<string, unknown>);

/** An HTML file and the pages it links to, in a sandboxed frame. The frame gets scripts but
 * an opaque origin, so it can't reach the app. The scheme's CSP blocks fetches, XHR,
 * websockets and forms off it, but not script or style URLs, or the frame navigating itself.
 * Reloads when a file it loaded changes. */
export const HtmlView = forwardRef<{ focus(): void }, Props>(function HtmlView(
  { workspace, path, toggle, saved, save },
  ref,
) {
  const frame = useRef<HTMLIFrameElement>(null);
  const findInput = useRef<HTMLInputElement>(null);
  const [root, setRoot] = useState<string>();
  const [trail, setTrail] = useState<Trail>({ urls: [], at: 0 });
  const [zoom, setZoom] = useState(typeof saved.zoom === "number" ? saved.zoom : 1);
  const [find, setFind] = useState<string>();
  const [notFound, setNotFound] = useState(false);
  const [error, setError] = useState<string>();
  /** Where the page was scrolled before a reload. */
  const scroll = useRef<{ x: number; y: number }>(undefined);
  const current = trail.urls[trail.at];

  useImperativeHandle(ref, () => ({ focus: () => frame.current?.focus() }), []);

  /** The saved page and scroll, until the first page shows. */
  const restoring = useRef<typeof saved | undefined>(saved);
  const saveRef = useRef(save);
  saveRef.current = save;
  /** The start of this tab's page URLs, before the page's own path. */
  const prefix = useRef("");

  useEffect(() => {
    let live = true;
    unwrap(commands.htmlRoot(workspace)).then(
      (r) => {
        if (!live) return;
        prefix.current = new URL(`${base()}${r}/`).href;
        const back = restoring.current;
        restoring.current = undefined;
        const url = back?.page ? new URL(back.page, prefix.current).href : pageUrl(base(), r, path);
        if (back?.pageScroll) scroll.current = { x: back.pageScroll[0], y: back.pageScroll[1] };
        setRoot(r);
        setTrail({ urls: [url], at: 0 });
        if (frame.current) frame.current.src = url;
      },
      (e) => live && setError(errorText(e)),
    );
    return () => {
      live = false;
    };
  }, [workspace, path]);

  function show(url: string) {
    if (frame.current) frame.current.src = url;
  }

  const post = (message: object) => send(frame.current, message);

  // Latest state for the message listener.
  const zoomRef = useRef(zoom);
  zoomRef.current = zoom;
  /** An open confirm is up, so the page's next asks are dropped rather than queued. */
  const confirming = useRef(false);

  useEffect(() => {
    const onMessage = (e: MessageEvent) => {
      if (!frame.current || e.source !== frame.current.contentWindow) return;
      const m = e.data as FrameMessage & { lgtm?: unknown };
      if (m?.lgtm !== true) return;
      switch (m.type) {
        case "nav":
          setTrail((t) => visit(t, m.url));
          if (m.url.startsWith(prefix.current)) {
            saveRef.current({ page: m.url.slice(prefix.current.length), pageScroll: undefined });
          }
          if (zoomRef.current !== 1) send(frame.current, { type: "zoom", zoom: zoomRef.current });
          if (scroll.current) send(frame.current, { type: "scroll", ...scroll.current });
          scroll.current = undefined;
          break;
        case "open": {
          // A page can post this without a click, so the URL is shown before it opens.
          const url = webUrl(m.url);
          if (!url || confirming.current) break;
          confirming.current = true;
          confirmAction(url, "Open")
            .then((ok) => ok && openUrl(url))
            .finally(() => {
              confirming.current = false;
            });
          break;
        }
        case "zoom":
          setZoom(fitted(m.zoom));
          break;
        case "found":
          setNotFound(!m.found);
          break;
        case "scroll":
          scroll.current = { x: m.x, y: m.y };
          break;
        case "scrolled":
          saveRef.current({ pageScroll: [Math.round(m.x), Math.round(m.y)] });
          break;
        case "key": {
          const init = appKey(m, document.activeElement === frame.current);
          if (!init) break;
          frame.current.dispatchEvent(
            new KeyboardEvent("keydown", { ...init, bubbles: true, cancelable: true }),
          );
          break;
        }
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);

  useEffect(() => {
    send(frame.current, { type: "zoom", zoom });
    saveRef.current({ zoom: zoom === 1 ? undefined : zoom });
  }, [zoom]);

  useRepoChanged(async () => {
    if (root && (await commands.htmlChanged(root))) reload();
  });

  /** Reloads the page, keeping its scroll position when its script still runs. */
  function reload() {
    scroll.current = undefined;
    post({ type: "reload" });
  }

  function go(by: 1 | -1) {
    const at = trail.at + by;
    if (at < 0 || at >= trail.urls.length) return;
    setTrail({ ...trail, at });
    show(trail.urls[at]);
  }

  function openFind() {
    setFind((f) => f ?? "");
    requestAnimationFrame(() => findInput.current?.select());
  }

  function closeFind() {
    setFind(undefined);
    setNotFound(false);
    frame.current?.focus();
  }

  function search(text: string, back: boolean, fresh: boolean) {
    if (text) post({ type: "find", text, back, fresh });
    else setNotFound(false);
  }

  function onKeyDown(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    const inField = e.target instanceof HTMLInputElement;
    let run: (() => void) | undefined;
    if (e.key === "Escape" && find !== undefined) run = closeFind;
    else if (zoomKey(e) === "in") run = () => setZoom((z) => stepZoom(z, 1));
    else if (zoomKey(e) === "out") run = () => setZoom((z) => stepZoom(z, -1));
    else if (zoomKey(e) === "actual") run = () => setZoom(1);
    else if (zoomKey(e) === "fit") run = () => post({ type: "fit" });
    else if (mod && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "r") run = reload;
    else if (mod && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "f") run = openFind;
    else if (mod && !inField && e.key === "ArrowLeft") run = () => go(-1);
    else if (mod && !inField && e.key === "ArrowRight") run = () => go(1);
    if (!run) return;
    e.preventDefault();
    e.stopPropagation();
    run();
  }

  const shown = root && current ? (pagePath(base(), root, current) ?? current) : path;

  return (
    <div className="html-view" onKeyDown={onKeyDown}>
      <div className="editor-path">
        <IconButton
          icon="chevronLeft"
          label="Back (⌘←)"
          disabled={trail.at === 0}
          onClick={() => go(-1)}
        />
        <IconButton
          icon="chevronRight"
          label="Forward (⌘→)"
          disabled={trail.at >= trail.urls.length - 1}
          onClick={() => go(1)}
        />
        <IconButton icon="retry" label="Reload (⌘R)" disabled={!current} onClick={reload} />
        <span className="path" title={shown}>
          {shown}
        </span>
        <ZoomControls
          zoom={zoom}
          setZoom={setZoom}
          fit={() => post({ type: "fit" })}
          fitLabel="Fit to Width"
        />
        {toggle}
      </div>
      {find !== undefined && (
        <div className="editor-bar html-find">
          <input
            ref={findInput}
            className={notFound ? "not-found" : undefined}
            value={find}
            placeholder="Find"
            spellCheck={false}
            onChange={(e) => {
              setFind(e.target.value);
              search(e.target.value, false, true);
            }}
            onKeyDown={(e) => {
              if (e.key !== "Enter") return;
              e.preventDefault();
              search(find, e.shiftKey, false);
            }}
          />
          <IconButton
            icon="chevronLeft"
            label="Previous (⇧↩)"
            onClick={() => search(find, true, false)}
          />
          <IconButton
            icon="chevronRight"
            label="Next (↩)"
            onClick={() => search(find, false, false)}
          />
          <IconButton icon="close" label="Close (Esc)" onClick={closeFind} />
        </div>
      )}
      {error && <div className="editor-bar error">{error}</div>}
      <iframe ref={frame} className="html-frame" title={path} sandbox="allow-scripts" />
    </div>
  );
});
