/** Paths, history and zoom for HTML tabs, which show pages from the `lgtmhtml:` scheme. */

export const isHtml = (path: string) => /\.html?$/i.test(path);

/** The URL of `path` under a root id, from the scheme's base URL. */
export function pageUrl(base: string, root: string, path: string): string {
  return new URL(`${base}${root}/${path.split("/").map(encodeURIComponent).join("/")}`).href;
}

/** The workspace path a page URL shows, with its #fragment, or undefined off the root. */
export function pagePath(base: string, root: string, url: string): string | undefined {
  const prefix = new URL(`${base}${root}/`).href;
  if (!url.startsWith(prefix)) return undefined;
  const rest = url.slice(prefix.length).replace(/\?[^#]*/, "");
  try {
    return decodeURIComponent(rest);
  } catch {
    return rest;
  }
}

/** The pages visited in a tab, and which one shows. */
export interface Trail {
  urls: string[];
  at: number;
}

/** `trail` after showing `url`: a new page drops the pages ahead, as in a browser. */
export function visit(trail: Trail, url: string): Trail {
  if (trail.urls[trail.at] === url) return trail;
  return { urls: [...trail.urls.slice(0, trail.at + 1), url], at: trail.at + 1 };
}

const ZOOMS = [0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 3];

/** The next zoom step in or out from `zoom`, which may sit between steps after a fit. */
export function stepZoom(zoom: number, by: 1 | -1): number {
  const next =
    by > 0
      ? ZOOMS.find((z) => z > zoom + 0.001)
      : [...ZOOMS].reverse().find((z) => z < zoom - 0.001);
  return next ?? zoom;
}

/** A fitted zoom kept in a usable range. */
export const fitted = (zoom: number) => Math.min(Math.max(zoom, 0.1), 3);
