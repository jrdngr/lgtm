// Runs first in every HTML page an HTML tab shows. The page sits in a sandboxed frame with an
// opaque origin, so this only talks to the tab through postMessage.
(() => {
  // A page's scripts can replace `parent`, and anything on a prototype, once they run, so
  // what the key path calls is kept here first.
  const host = parent;
  const apply = Reflect.apply;
  const getter = (proto, name) => Object.getOwnPropertyDescriptor(proto, name).get;
  const send = MessagePort.prototype.postMessage;
  const dataOf = getter(MessageEvent.prototype, "data");
  const [key, code, metaKey, ctrlKey, shiftKey, altKey] = [
    "key", "code", "metaKey", "ctrlKey", "shiftKey", "altKey",
  ].map((name) => getter(KeyboardEvent.prototype, name));

  const post = (m) => host.postMessage({ lgtm: true, ...m }, "*");
  const nav = () => post({ type: "nav", url: location.href, title: document.title });
  addEventListener("DOMContentLoaded", nav);
  addEventListener("hashchange", nav);

  // Links off this page's files open in the browser.
  addEventListener(
    "click",
    (e) => {
      const a = e.target instanceof Element && e.target.closest("a[href]");
      if (!a || e.defaultPrevented) return;
      const url = new URL(a.href, location.href);
      if (url.protocol === "http:" || url.protocol === "https:") {
        if (url.origin === location.origin) return;
        e.preventDefault();
        post({ type: "open", url: url.href });
      }
    },
    true,
  );

  // App shortcuts and Escape the user types go to the tab, over a port the tab hands this
  // script each time the frame loads, so a page's scripts can't send keys. This listener is
  // the page's first, and keeps the port from them. Editing shortcuts stay in the page. The
  // tab checks the same rule (appKey in htmlNav.ts).
  const [sourceOf, portsOf] = ["source", "ports"].map((name) =>
    getter(MessageEvent.prototype, name),
  );
  const stop = Event.prototype.stopImmediatePropagation;
  let keys;
  addEventListener(
    "message",
    (e) => {
      const m = apply(dataOf, e, []);
      if (apply(sourceOf, e, []) !== host || m?.lgtm !== true || m.type !== "port") return;
      apply(stop, e, []);
      keys = apply(portsOf, e, [])[0];
    },
    true,
  );
  addEventListener(
    "keydown",
    (e) => {
      if (!e.isTrusted) return;
      const k = apply(key, e, []);
      const mod = apply(metaKey, e, []) || apply(ctrlKey, e, []);
      if (!mod && k !== "Escape") return;
      const plain = !apply(shiftKey, e, []) && !apply(altKey, e, []);
      if (mod && plain && ["a", "c", "v", "x", "z"].includes(k.toLowerCase())) return;
      e.preventDefault();
      if (!keys) return;
      apply(send, keys, [
        {
          key: k,
          code: apply(code, e, []),
          metaKey: apply(metaKey, e, []),
          ctrlKey: apply(ctrlKey, e, []),
          shiftKey: apply(shiftKey, e, []),
          altKey: apply(altKey, e, []),
        },
      ]);
    },
    true,
  );

  addEventListener("contextmenu", (e) => e.preventDefault());

  // Where the page is scrolled, once scrolling stops, so the tab can bring it back.
  let scrolling;
  addEventListener("scroll", () => {
    clearTimeout(scrolling);
    scrolling = setTimeout(() => post({ type: "scrolled", x: scrollX, y: scrollY }), 200);
  });

  /** The zoom that fits the page's content to the frame's width. */
  function fitZoom() {
    const root = document.documentElement;
    root.style.zoom = "";
    let right = 0;
    let left = Number.POSITIVE_INFINITY;
    const parts = "svg, img, canvas, table, pre";
    for (const el of document.querySelectorAll(parts)) {
      if (el.parentElement?.closest(parts)) continue;
      const r = el.getBoundingClientRect();
      if (r.width === 0) continue;
      right = Math.max(right, r.right + scrollX);
      left = Math.min(left, r.left + scrollX);
    }
    const width = right > 0 ? right + Math.max(left, 0) : root.scrollWidth;
    return width > 0 ? root.clientWidth / width : 1;
  }

  addEventListener("message", (e) => {
    const m = e.data;
    if (e.source !== parent || !m || m.lgtm !== true) return;
    if (m.type === "zoom") document.documentElement.style.zoom = m.zoom === 1 ? "" : String(m.zoom);
    if (m.type === "fit") post({ type: "zoom", zoom: fitZoom() });
    if (m.type === "find") {
      if (m.fresh) getSelection()?.removeAllRanges();
      const found = m.text ? window.find(m.text, false, m.back, true) : false;
      post({ type: "found", found });
    }
    // Reloads keep the scroll position: the tab hands it back after the reload.
    if (m.type === "reload") {
      post({ type: "scroll", x: scrollX, y: scrollY });
      location.reload();
    }
    if (m.type === "scroll") {
      const go = () => scrollTo(m.x, m.y);
      go();
      if (document.readyState !== "complete") addEventListener("load", go, { once: true });
    }
  });
})();
