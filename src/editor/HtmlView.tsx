import { convertFileSrc } from "@tauri-apps/api/core";
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { isMac } from "../core/keys";
import type { WorkspaceName } from "../core/workspace";
import { FindBar, type FindState, stepKey } from "../find/FindBar";
import { commands, errorText, unwrap } from "../ipc";
import { openUrl } from "../services/pathMenu";
import { IconButton } from "../ui/icons";
import { confirmAction } from "../ui/prompt";
import { useRepoChanged } from "../workspaces/useRepoChanged";
import { appKey, fitted, pagePath, pageUrl, stepZoom, type Trail, visit, webUrl } from "./htmlNav";
import { keyPort } from "./keyPort";
import type { EditorParams } from "./open";
import { ZoomControls, zoomKey } from "./ZoomControls";

const base = () => convertFileSrc("", "lgtmhtml");

/** An open confirm is up in some HTML tab, so pages' next asks are dropped rather than queued. */
let confirming = false;

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
  | { type: "pinch"; factor: number; x: number; y: number }
  | { type: "found"; at: number; total: number }
  | { type: "scroll"; x: number; y: number }
  | { type: "scrolled"; x: number; y: number };

/** An HTML file and the pages it links to, in a sandboxed frame. The frame gets scripts but
 * an opaque origin, so it can't reach the app. The scheme's CSP blocks fetches, XHR,
 * websockets and forms off it, but not script or style URLs, or the frame navigating itself.
 * Reloads when a file it loaded changes. */
export const HtmlView = forwardRef<{ focus(): void }, Props>(function HtmlView(
  { workspace, path, toggle, saved, save },
  ref,
) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [root, setRoot] = useState<string>();
  const [trail, setTrail] = useState<Trail>({ urls: [], at: 0 });
  const [zoom, setZoom] = useState(typeof saved.zoom === "number" ? saved.zoom : 1);
  const [find, setFind] = useState<FindState>();
  const [found, setFound] = useState({ at: 0, total: 0 });
  const [findFocus, setFindFocus] = useState(0);
  const [error, setError] = useState<string>();
  /** The user cancelled an open, so the page's asks are dropped until it loads again. */
  const refused = useRef(false);
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

  // App keys come from the frame script over a port; see keyPort.
  const [keys] = useState(() =>
    keyPort((m) => {
      const init = appKey(m, document.activeElement === frame.current);
      if (!init) return;
      frame.current?.dispatchEvent(
        new KeyboardEvent("keydown", { ...init, bubbles: true, cancelable: true }),
      );
    }),
  );
  useEffect(() => () => keys.close(), [keys]);

  /** Bumped to show a page in a new frame, so no script from the last one is left. */
  const [generation, setGeneration] = useState(0);
  const next = useRef({ url: "", focus: false });
  function show(url: string) {
    next.current = { url, focus: document.activeElement === frame.current };
    refused.current = false;
    setGeneration((g) => g + 1);
  }
  useLayoutEffect(() => {
    if (generation === 0 || !frame.current) return;
    keys.close();
    frame.current.src = next.current.url;
    if (next.current.focus) frame.current.focus();
  }, [generation, keys]);

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
        next.current = { url, focus: false };
        setGeneration((g) => g + 1);
      },
      (e) => live && setError(errorText(e)),
    );
    return () => {
      live = false;
    };
  }, [workspace, path]);

  const post = (message: object) => send(frame.current, message);

  // Latest state for the message listener.
  const zoomRef = useRef(zoom);
  zoomRef.current = zoom;

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
          if (!url || confirming || refused.current) break;
          confirming = true;
          confirmAction(url, "Open")
            .then((ok) => {
              if (ok) openUrl(url);
              else refused.current = true;
            })
            .finally(() => {
              confirming = false;
            });
          break;
        }
        case "zoom":
          setZoom(fitted(m.zoom));
          break;
        case "pinch": {
          if (![m.factor, m.x, m.y].every(Number.isFinite)) break;
          const next = fitted(zoomRef.current * m.factor);
          zoomRef.current = next;
          send(frame.current, { type: "zoom", zoom: next, x: m.x, y: m.y });
          setZoom(next);
          break;
        }
        case "found":
          setFound({ at: m.at, total: m.total });
          break;
        case "scroll":
          scroll.current = { x: m.x, y: m.y };
          break;
        case "scrolled":
          saveRef.current({ pageScroll: [Math.round(m.x), Math.round(m.y)] });
          break;
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
  }, workspace);

  /** A page loaded in the frame: a load a page can't fake. */
  function onLoad() {
    refused.current = false;
    const win = frame.current?.contentWindow;
    keys.connect((port) => win?.postMessage({ lgtm: true, type: "port" }, "*", [port]));
  }

  /** Reloads the page, keeping its scroll position when its script still runs. */
  function reload() {
    scroll.current = undefined;
    refused.current = false;
    post({ type: "reload" });
  }

  function go(by: 1 | -1) {
    const at = trail.at + by;
    if (at < 0 || at >= trail.urls.length) return;
    setTrail({ ...trail, at });
    show(trail.urls[at]);
  }

  function openFind() {
    setFind((f) => f ?? { query: "", matchCase: false });
    setFindFocus((k) => k + 1);
  }

  function closeFind() {
    setFind(undefined);
    setFound({ at: 0, total: 0 });
    post({ type: "findClose" });
    frame.current?.focus();
  }

  function search(next: FindState, back: boolean, fresh: boolean) {
    post({ type: "find", text: next.query, matchCase: next.matchCase, back, fresh });
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
    else if (find !== undefined && stepKey(e) !== undefined)
      run = () => search(find, stepKey(e) ?? false, false);
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
        <div className="find-anchor">
          <FindBar
            {...find}
            {...found}
            focusKey={findFocus}
            onChange={(next) => {
              setFind(next);
              search(next, false, true);
            }}
            onStep={(back) => search(find, back, false)}
            onClose={closeFind}
          />
        </div>
      )}
      {error && <div className="editor-bar error">{error}</div>}
      <iframe
        key={generation}
        ref={frame}
        className="html-frame"
        title={path}
        sandbox="allow-scripts"
        onLoad={onLoad}
      />
    </div>
  );
});
