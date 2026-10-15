const TYPES: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  avif: "image/avif",
  bmp: "image/bmp",
  ico: "image/x-icon",
  icns: "image/x-icns",
  tif: "image/tiff",
  tiff: "image/tiff",
  heic: "image/heic",
};

/** The image type of a file the image view shows, by extension. SVG is text and opens in
 * the editor. */
export function imageType(path: string): string | undefined {
  const dot = path.lastIndexOf(".");
  return dot < 0 ? undefined : TYPES[path.slice(dot + 1).toLowerCase()];
}

export const isSvg = (path: string) => /\.svg$/i.test(path);

/** An SVG's text as an image URL, which shows it without running its scripts. */
export const svgUrl = (text: string) =>
  `data:image/svg+xml;charset=utf-8,${encodeURIComponent(text)}`;
