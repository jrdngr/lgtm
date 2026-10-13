import type { ReactNode } from "react";
import type { QText } from "../bindings";
import { Inline } from "../bots/Markdown";

interface Props {
  id: string;
  text: QText;
  /** What it said before a rewrite: parts that differ are marked amber. */
  before?: QText;
  /** The option picked out, from 0. */
  selected?: number;
  /** The option it was decided with. */
  decided?: number | null;
  /** Options can't be picked. */
  locked?: boolean;
  onSelect?(option: number): void;
  onDecide?(option: number): void;
  /** Shown under option `i`, like the note being written for it. */
  under?(i: number): ReactNode;
  className?: string;
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** A question in the qqueue template: subject, context, options with the recommended one
 * first, and the recommendation. */
export function QuestionCard({ id, text, before, selected, decided, locked, ...p }: Props) {
  const changed = (now: unknown, was: unknown) => (before && !same(now, was) ? " qq-changed" : "");
  return (
    <article className={`qq-card${p.className ? ` ${p.className}` : ""}`}>
      <h2 className="qq-subject">
        <span className="qq-id">{id}</span> —{" "}
        <span className={changed(text.subject, before?.subject)}>
          <Inline>{text.subject}</Inline>
        </span>
      </h2>
      <div className="qq-label">Context</div>
      <ul className="qq-context">
        {text.context.map((c) => (
          <li key={c} className={changed(true, before?.context.includes(c))}>
            <Inline>{c}</Inline>
          </li>
        ))}
      </ul>
      <div className="qq-label">Options</div>
      <ol className="qq-options">
        {text.options.map((o, i) => (
          <li key={o.label}>
            <button
              type="button"
              className={[
                "qq-option",
                i === 0 && "recommended",
                selected === i && "selected",
                decided === i && "decided",
                changed(o, before?.options[i]).trim(),
              ]
                .filter(Boolean)
                .join(" ")}
              disabled={locked || !p.onSelect}
              onClick={() => p.onSelect?.(i)}
              onDoubleClick={() => p.onDecide?.(i)}
            >
              <span className="qq-num">{i + 1}</span>
              <span className="qq-option-body">
                <span className="qq-option-label">
                  <b>
                    <Inline>{o.label}</Inline>
                  </b>
                  {i === 0 && <span className="pill green">recommended</span>}
                  {decided === i && <span className="pill blue">your answer</span>}
                </span>
                <ul>
                  {o.bullets.map((b) => (
                    <li key={b}>
                      <Inline>{b}</Inline>
                    </li>
                  ))}
                </ul>
              </span>
            </button>
            {p.under?.(i)}
          </li>
        ))}
      </ol>
      <p className={`qq-rec${changed(text.why, before?.why)}`}>
        <b>Recommendation:</b> option 1. <Inline>{text.why}</Inline>
      </p>
    </article>
  );
}
