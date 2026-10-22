import type { Hit } from "../bindings";

/** `text` with the `spans` (UTF-16 ranges) marked. */
export function Marked({ text, spans }: { text: string; spans: [number, number][] }) {
  const parts: React.ReactNode[] = [];
  let at = 0;
  for (const [s, e] of spans) {
    if (s < at) continue;
    parts.push(text.slice(at, s));
    parts.push(<mark key={s}>{text.slice(s, e)}</mark>);
    at = e;
  }
  parts.push(text.slice(at));
  return <>{parts}</>;
}

interface Props {
  hit: Hit;
  on: boolean;
  /** The review changes this line. */
  changed?: boolean;
  /** Shows the lines around it too. */
  context?: boolean;
  onOpen(): void;
}

/** One matching line: its number and text, the matches marked. */
export function HitLine({ hit, on, changed, context, onOpen }: Props) {
  const row = (
    // biome-ignore lint/a11y/useKeyWithClickEvents: the list takes the keys
    <div
      className={`sr-hit${on ? " on" : ""}${changed ? " chg" : ""}`}
      data-on={on || undefined}
      onClick={onOpen}
    >
      <span className="sr-ln">{hit.line}</span>
      <span className="sr-tx">
        <Marked text={hit.text} spans={hit.spans} />
      </span>
    </div>
  );
  if (!context) return row;
  return (
    <div className="sr-snip">
      {hit.before !== null && (
        <div className="sr-hit ctx">
          <span className="sr-ln">{hit.line - 1}</span>
          <span className="sr-tx">{hit.before}</span>
        </div>
      )}
      {row}
      {hit.after !== null && (
        <div className="sr-hit ctx">
          <span className="sr-ln">{hit.line + 1}</span>
          <span className="sr-tx">{hit.after}</span>
        </div>
      )}
    </div>
  );
}

/** `path` as its file name, then its folder. */
export function FileName({ path }: { path: string }) {
  const at = path.lastIndexOf("/");
  return (
    <>
      <span className="sr-name">{path.slice(at + 1)}</span>
      {at > 0 && <span className="sr-dir">{path.slice(0, at)}</span>}
    </>
  );
}

/** Hits grouped by path, in the order they come. */
export function byFile(hits: Hit[]): [string, Hit[]][] {
  const out = new Map<string, Hit[]>();
  for (const h of hits) out.set(h.path, [...(out.get(h.path) ?? []), h]);
  return [...out];
}

/** As `path:line: text` lines, for sending to an agent. */
export const hitsText = (hits: Hit[]) =>
  hits.map((h) => `${h.path}:${h.line}: ${h.text.trim()}`).join("\n");
