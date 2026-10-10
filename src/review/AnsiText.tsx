import { parseAnsi } from "./ansi";

/** Text with ANSI colors, as jj prints it. */
export function AnsiText({ text }: { text: string }) {
  return (
    <>
      {parseAnsi(text).map((s, i) => (
        <span
          // biome-ignore lint/suspicious/noArrayIndexKey: spans have no identity
          key={i}
          style={{ color: s.color, fontWeight: s.bold ? "bold" : undefined }}
        >
          {s.text}
        </span>
      ))}
    </>
  );
}
