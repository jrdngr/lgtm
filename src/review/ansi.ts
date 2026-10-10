/** A run of text with the style ANSI SGR codes gave it. */
export interface Span {
  text: string;
  color?: string;
  bold?: boolean;
}

const BASE = [
  "#3b3b3b",
  "#f85149",
  "#3fb950",
  "#d29922",
  "#4f8cc9",
  "#c586c0",
  "#39c5cf",
  "#d4d4d4",
  "#8a8a8a",
  "#ff7b72",
  "#56d364",
  "#e3b341",
  "#79c0ff",
  "#d2a8ff",
  "#56d4dd",
  "#ffffff",
];

function color256(n: number): string {
  if (n < 16) return BASE[n];
  if (n >= 232) {
    const v = 8 + (n - 232) * 10;
    return `rgb(${v} ${v} ${v})`;
  }
  const i = n - 16;
  const level = (c: number) => (c === 0 ? 0 : 55 + c * 40);
  return `rgb(${level(Math.floor(i / 36))} ${level(Math.floor(i / 6) % 6)} ${level(i % 6)})`;
}

/** Splits text with SGR escapes (as `jj --color=always` prints) into styled spans. */
export function parseAnsi(input: string): Span[] {
  const out: Span[] = [];
  let style: Omit<Span, "text"> = {};
  // biome-ignore lint/suspicious/noControlCharactersInRegex: matching ESC is the point
  const re = /\x1b\[([0-9;]*)m/g;
  let last = 0;
  for (let m = re.exec(input); m; m = re.exec(input)) {
    if (m.index > last) out.push({ text: input.slice(last, m.index), ...style });
    last = re.lastIndex;
    const codes = m[1] === "" ? [0] : m[1].split(";").map(Number);
    for (let i = 0; i < codes.length; i++) {
      const c = codes[i];
      if (c === 0) style = {};
      else if (c === 1) style = { ...style, bold: true };
      else if (c === 22) style = { ...style, bold: false };
      else if (c === 39) style = { ...style, color: undefined };
      else if (c >= 30 && c <= 37) style = { ...style, color: BASE[c - 30] };
      else if (c >= 90 && c <= 97) style = { ...style, color: BASE[c - 90 + 8] };
      else if (c === 38 && codes[i + 1] === 5) {
        style = { ...style, color: color256(codes[i + 2]) };
        i += 2;
      }
    }
  }
  if (last < input.length) out.push({ text: input.slice(last), ...style });
  return out;
}
