/** Tab titles show whole while they fit. Once a pane's tabs run out of room, titles share a
 * width cap, so the longest ones shorten first; CSS keeps the cap at 10 characters or more. */

/** The widest a title can be for titles `widths` to fit in `room`, or undefined when they all
 * fit whole. Titles narrower than the cap keep their width. */
export function titleCap(widths: number[], room: number): number | undefined {
  const sorted = [...widths].sort((a, b) => a - b);
  let left = room;
  for (let i = 0; i < sorted.length; i++) {
    const n = sorted.length - i;
    if (sorted[i] * n > left) return Math.max(0, left / n);
    left -= sorted[i];
  }
  return undefined;
}

/** No cap: wider than any title. */
const WHOLE = "100000px";
const watched = new WeakSet<Element>();
let resized: ResizeObserver | undefined;
let frame = 0;

/** Refits when `el` changes size: a header resizing, or a pane's tabs growing or shrinking. */
function watch(el: Element) {
  if (watched.has(el)) return;
  watched.add(el);
  resized ??= new ResizeObserver(() => fitTabs());
  resized.observe(el);
}

/** Refits every pane's tabs on the next frame. */
export function fitTabs() {
  if (frame) return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    for (const tabs of document.querySelectorAll<HTMLElement>(".dock .dv-tabs-container"))
      fitPane(tabs);
    requestAnimationFrame(recountOverflow);
  });
}

/** Dockview counts hidden tabs only when the tab list's box resizes or scrolls, so a fit that
 * leaves the box the same size would keep a stale count. A scroll event makes it recount. */
function recountOverflow() {
  for (const tabs of document.querySelectorAll<HTMLElement>(".dock .dv-tabs-container"))
    tabs.dispatchEvent(new Event("scroll"));
}

function fitPane(tabs: HTMLElement) {
  const header = tabs.closest(".dv-tabs-and-actions-container");
  if (!header) return;
  watch(header);
  watch(tabs);
  // The header's room, less what its other parts take; the empty filler gives way. A little
  // slack covers rounding.
  let room = header.clientWidth - 2;
  for (const part of header.children) {
    if (part.contains(tabs) || part.classList.contains("dv-void-container")) continue;
    room -= part.getBoundingClientRect().width;
  }
  const widths: number[] = [];
  for (const tab of tabs.querySelectorAll<HTMLElement>(".dv-tab")) {
    const title = tab.querySelector<HTMLElement>(".dv-default-tab-content");
    if (!title) {
      room -= tab.getBoundingClientRect().width;
      continue;
    }
    room -= tab.getBoundingClientRect().width - title.clientWidth;
    widths.push(title.scrollWidth);
  }
  const cap = titleCap(widths, room);
  tabs.style.setProperty("--tab-title-max", cap === undefined ? WHOLE : `${Math.floor(cap)}px`);
}

if (typeof window !== "undefined") window.addEventListener("resize", fitTabs);
