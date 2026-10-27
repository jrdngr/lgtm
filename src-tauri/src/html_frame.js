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

  // A trackpad pinch goes to the tab, which sets the zoom and hands back the point to keep
  // under the fingers. WebKit sends gesture events; other engines send ctrl+wheel.
  let pinchScale = 0;
  const pinch = (factor, x, y) => post({ type: "pinch", factor, x, y });
  addEventListener("gesturestart", (e) => {
    e.preventDefault();
    pinchScale = 1;
  });
  addEventListener("gesturechange", (e) => {
    e.preventDefault();
    if (!pinchScale || e.scale <= 0) return;
    pinch(e.scale / pinchScale, e.clientX, e.clientY);
    pinchScale = e.scale;
  });
  addEventListener("gestureend", (e) => {
    e.preventDefault();
    pinchScale = 0;
  });
  addEventListener(
    "wheel",
    (e) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      const px = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaMode === 2 ? e.deltaY * 400 : e.deltaY;
      if (!pinchScale) pinch(Math.exp(-Math.max(-50, Math.min(50, px)) * 0.01), e.clientX, e.clientY);
    },
    { passive: false, capture: true },
  );

  /** Sets the page's zoom, keeping what's at (x, y) there when given. Measures an element
   * rather than doing the sums, since engines differ on how a zoomed root scrolls. */
  function setZoom(zoom, x, y) {
    const el = typeof x === "number" ? document.elementFromPoint(x, y) : null;
    const before = el?.getBoundingClientRect();
    document.documentElement.style.zoom = zoom === 1 ? "" : String(zoom);
    if (!el || !before || !before.width || !before.height) return;
    const fx = (x - before.left) / before.width;
    const fy = (y - before.top) / before.height;
    const off = () => {
      const r = el.getBoundingClientRect();
      return [r.left + fx * r.width - x, r.top + fy * r.height - y];
    };
    const [dx, dy] = off();
    if (!dx && !dy) return;
    scrollBy(dx, dy);
    // Where scrolling by one unit doesn't move the page one pixel, scroll the rest scaled.
    const [rx, ry] = off();
    const left = (d, r) => (r && d !== r ? (r * d) / (d - r) : 0);
    scrollBy(left(dx, rx), left(dy, ry));
  }

  // Find: every match highlighted, the current one more strongly, with the count sent back.
  let found = { text: "", matchCase: false, ranges: [], at: 0 };

  function matches(text, matchCase) {
    const nodes = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {
      acceptNode: (n) =>
        n.parentElement?.closest("script, style, noscript")
          ? NodeFilter.FILTER_REJECT
          : NodeFilter.FILTER_ACCEPT,
    });
    for (let n = walker.nextNode(); n; n = walker.nextNode()) nodes.push(n);
    const starts = [];
    let pos = 0;
    for (const n of nodes) {
      starts.push(pos);
      pos += n.data.length;
    }
    const all = nodes.map((n) => n.data).join("");
    const hay = matchCase ? all : all.toLowerCase();
    const needle = matchCase ? text : text.toLowerCase();
    const at = (offset, end) => {
      let i = nodes.length - 1;
      while (i > 0 && (starts[i] > offset || (end && starts[i] === offset))) i--;
      return [nodes[i], offset - starts[i]];
    };
    const ranges = [];
    if (!needle) return ranges;
    for (let i = hay.indexOf(needle); i >= 0; i = hay.indexOf(needle, i + needle.length)) {
      const r = document.createRange();
      r.setStart(...at(i, false));
      r.setEnd(...at(i + needle.length, true));
      ranges.push(r);
    }
    return ranges;
  }

  // WebKit leaves ranges painted after their highlight is swapped out, so the two highlights
  // are made once and refilled in place.
  let marks;

  function paint() {
    if (!("highlights" in CSS)) return;
    if (!marks) {
      const style = document.createElement("style");
      style.textContent =
        "::highlight(lgtm-find){background-color:rgb(234 92 0 / 33%)}" +
        "::highlight(lgtm-find-current){background-color:rgb(255 170 0 / 70%);color:#1e1e1e}";
      document.head.append(style);
      marks = { all: new Highlight(), current: new Highlight() };
      CSS.highlights.set("lgtm-find", marks.all);
      CSS.highlights.set("lgtm-find-current", marks.current);
    }
    marks.all.clear();
    marks.current.clear();
    for (const r of found.ranges) marks.all.add(r);
    const current = found.ranges[found.at];
    if (current) marks.current.add(current);
  }

  function unpaint() {
    marks?.all.clear();
    marks?.current.clear();
  }

  function find(m) {
    const total = found.ranges.length;
    if (m.fresh || m.text !== found.text || m.matchCase !== found.matchCase) {
      found = { text: m.text, matchCase: !!m.matchCase, ranges: matches(m.text, !!m.matchCase), at: 0 };
    } else if (total) {
      found.at = (found.at + (m.back ? -1 : 1) + total) % total;
    }
    paint();
    found.ranges[found.at]?.startContainer.parentElement?.scrollIntoView({ block: "center" });
    post({ type: "found", at: found.at, total: found.ranges.length });
  }

  addEventListener("message", (e) => {
    const m = e.data;
    if (e.source !== parent || !m || m.lgtm !== true) return;
    if (m.type === "zoom") setZoom(m.zoom, m.x, m.y);
    if (m.type === "fit") post({ type: "zoom", zoom: fitZoom() });
    if (m.type === "find") find(m);
    if (m.type === "findClose") unpaint();
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
