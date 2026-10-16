// Runs first in every HTML page an HTML tab shows. The page sits in a sandboxed frame with an
// opaque origin, so this only talks to the tab through postMessage.
(() => {
  const post = (m) => parent.postMessage({ lgtm: true, ...m }, "*");
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

  // App shortcuts and Escape go to the tab. Editing shortcuts stay in the page.
  addEventListener(
    "keydown",
    (e) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod && e.key !== "Escape") return;
      if (mod && !e.shiftKey && !e.altKey && ["a", "c", "v", "x", "z"].includes(e.key.toLowerCase())) return;
      e.preventDefault();
      const { key, code, metaKey, ctrlKey, shiftKey, altKey } = e;
      post({ type: "key", key, code, metaKey, ctrlKey, shiftKey, altKey });
    },
    true,
  );

  addEventListener("contextmenu", (e) => e.preventDefault());

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
