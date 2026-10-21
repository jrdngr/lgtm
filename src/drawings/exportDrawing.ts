import { save } from "@tauri-apps/plugin-dialog";
import type { Swatch } from "../bindings";
import { shapePaint } from "../canvas/CanvasShapes";
import { NOTE_PX, noteBox, STICKY_PAD } from "../canvas/notes";
import { INK, swatchHex } from "../canvas/palette";
import type { useCanvasEdit } from "../canvas/useCanvasEdit";
import { commands, unwrap } from "../ipc";
import { tint } from "../system/colors";
import { arrowPath } from "../system/edgePaths";
import type { Box } from "../system/layout";

type Ed = ReturnType<typeof useCanvasEdit>;

const BG = "#161719";
const PAD = 32;
const FONT = "-apple-system, BlinkMacSystemFont, 'SF Pro Text', system-ui, sans-serif";

const esc = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const hexOf = (c: Swatch | null | undefined, fallback: string) => (c ? swatchHex(c) : fallback);

/** Lines of `text`, each a `<tspan>` centered on `x`, the block centered on `cy`. */
function lines(text: string, x: number, cy: number, px: number, anchor = "middle") {
  const rows = text.split("\n");
  const top = cy - ((rows.length - 1) * px * 1.35) / 2;
  return rows
    .map(
      (r, i) =>
        `<tspan x="${x}" y="${top + i * px * 1.35}" text-anchor="${anchor}" dominant-baseline="central">${esc(r)}</tspan>`,
    )
    .join("");
}

/** The drawing as a standalone SVG, cropped to what's on it. */
export function drawingSvg(ed: Ed): string {
  const v = ed.view;
  const boxes: Box[] = [
    ...v.shapes.map((s) => ({ x: s.at.x, y: s.at.y, w: s.w, h: s.h })),
    ...v.notes.map(noteBox),
    ...ed.frames.values(),
  ];
  const ends = v.arrows.flatMap((a) => [ed.endPoint(a.from, a.to), ed.endPoint(a.to, a.from)]);
  const xs = [...boxes.flatMap((b) => [b.x, b.x + b.w]), ...ends.map((p) => p.x)];
  const ys = [...boxes.flatMap((b) => [b.y - 12, b.y + b.h]), ...ends.map((p) => p.y)];
  const x0 = (xs.length ? Math.min(...xs) : 0) - PAD;
  const y0 = (ys.length ? Math.min(...ys) : 0) - PAD;
  const w = (xs.length ? Math.max(...xs) : 0) + PAD - x0;
  const h = (ys.length ? Math.max(...ys) : 0) + PAD - y0;
  const out: string[] = [];
  for (const [id, b] of [...ed.frames].sort(([, a], [, c]) => a.depth - c.depth)) {
    const g = v.groups.find((x) => x.id === id);
    const c = hexOf(g?.color, "#8b8d93");
    out.push(
      `<rect x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" rx="14" fill="${tint(c, 0.05)}" stroke="${tint(c, 0.5)}" stroke-width="1.5"/>`,
      `<text x="${b.x + 16}" y="${b.y}" fill="${c}" font-size="11.5" font-weight="600" dominant-baseline="central">${esc(g?.label ?? "")}</text>`,
    );
  }
  for (const s of v.shapes) {
    const { stroke, fill } = shapePaint(s);
    const dash = s.pattern !== "solid" ? ' stroke-dasharray="7 5"' : "";
    const paint = `fill="${fill === "transparent" ? "none" : fill}" stroke="${stroke}" stroke-width="1.5"${dash}`;
    if (s.kind === "rect")
      out.push(
        `<rect x="${s.at.x}" y="${s.at.y}" width="${s.w}" height="${s.h}" rx="8" ${paint}/>`,
      );
    else if (s.kind === "ellipse")
      out.push(
        `<ellipse cx="${s.at.x + s.w / 2}" cy="${s.at.y + s.h / 2}" rx="${s.w / 2}" ry="${s.h / 2}" ${paint}/>`,
      );
    else
      out.push(
        `<polygon points="${s.at.x + s.w / 2},${s.at.y} ${s.at.x + s.w},${s.at.y + s.h / 2} ${s.at.x + s.w / 2},${s.at.y + s.h} ${s.at.x},${s.at.y + s.h / 2}" ${paint}/>`,
      );
    if (s.text)
      out.push(
        `<text fill="#ececec" font-size="13" font-weight="600">${lines(s.text, s.at.x + s.w / 2, s.at.y + s.h / 2, 13)}</text>`,
      );
  }
  for (const a of v.arrows) {
    const c = hexOf(a.color, INK);
    const p = ed.endPoint(a.from, a.to);
    const q = ed.endPoint(a.to, a.from);
    const dash = a.pattern === "dashed" ? ' stroke-dasharray="9 7"' : "";
    const head = a.head !== false ? ` marker-end="url(#h${c.slice(1)})"` : "";
    out.push(
      `<path d="${arrowPath(p, q)}" fill="none" stroke="${c}" stroke-width="3" stroke-linecap="round"${dash}${head}/>`,
    );
  }
  for (const n of v.notes) {
    const px = NOTE_PX[n.size];
    const b = noteBox(n);
    if (n.sticky) {
      const c = hexOf(n.color, swatchHex("yellow"));
      out.push(
        `<rect x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" rx="8" fill="${tint(c, 0.18)}" stroke="${tint(c, 0.45)}"/>`,
        `<text fill="#ececec" font-size="${px}" font-weight="500">${lines(n.text, b.x + STICKY_PAD, b.y + b.h / 2, px, "start")}</text>`,
      );
    } else {
      out.push(
        `<text fill="${hexOf(n.color, INK)}" font-size="${px}" font-weight="600">${lines(n.text, b.x, b.y + b.h / 2, px, "start")}</text>`,
      );
    }
  }
  const heads = [...new Set(v.arrows.map((a) => hexOf(a.color, INK)))].map(
    (c) =>
      `<marker id="h${c.slice(1)}" viewBox="0 0 12 12" refX="10" refY="6" markerWidth="12" markerHeight="12" markerUnits="userSpaceOnUse" orient="auto"><path d="M0 0L12 6L0 12z" fill="${c}"/></marker>`,
  );
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="${x0} ${y0} ${w} ${h}" font-family="${esc(FONT)}"><defs>${heads.join("")}</defs><rect x="${x0}" y="${y0}" width="${w}" height="${h}" fill="${BG}"/>${out.join("")}</svg>`;
}

/** The SVG drawn at twice its size, as PNG bytes. */
export async function svgToPng(svg: string): Promise<Blob> {
  const img = new Image();
  const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
  try {
    img.src = url;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = img.width * 2;
    c.height = img.height * 2;
    const g = c.getContext("2d");
    if (!g) throw new Error("no 2D canvas");
    g.scale(2, 2);
    g.drawImage(img, 0, 0);
    return await new Promise((ok, fail) =>
      c.toBlob((b) => (b ? ok(b) : fail(new Error("couldn't make the PNG"))), "image/png"),
    );
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** Asks where to save, then writes `data` there. */
export async function saveAs(name: string, ext: string, data: Blob | string) {
  const path = await save({
    defaultPath: `${name}.${ext}`,
    filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
  });
  if (!path) return;
  const bytes =
    typeof data === "string"
      ? new TextEncoder().encode(data)
      : new Uint8Array(await data.arrayBuffer());
  await unwrap(commands.writeExport(path, [...bytes]));
}

/** Puts the PNG on the clipboard. */
export function copyPng(png: Promise<Blob>) {
  return navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
}
