import type { ReactNode } from "react";
import type { Answer, QText } from "../bindings";
import { Inline } from "../bots/Markdown";
import { Icon } from "../ui/icons";

interface Props {
  id: string;
  text: QText;
  /** What it said before it reopened: parts that differ are marked amber. */
  before?: QText;
  /** The answer it had before it reopened. */
  was?: Answer | null;
  /** The option picked out, from 0. */
  selected?: number;
  /** The option it was decided with. */
  decided?: number | null;
  /** The agent decided it, so the answer shows as its own. */
  auto?: boolean;
  /** Options can't be picked. */
  locked?: boolean;
  onSelect?(option: number): void;
  onDecide?(option: number): void;
  /** Asks the agent to explain option `i`. No button without it. */
  onExplain?(option: number): void;
  /** Under the subject: what it depends on and what depends on it. */
  deps?: ReactNode;
  /** At the foot of the context: the files the agent read for it. */
  sources?: ReactNode;
  /** Shown under option `i`, like the note being written for it. */
  under?(i: number): ReactNode;
  className?: string;
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** An option's labeled tradeoffs, in the order they show. */
const TAGS = [
  ["Pro", "pros"],
  ["Con", "cons"],
  ["Risk", "risks"],
] as const;

/** A question in the qqueue template: subject, context, options with the recommended one
 * first, and the recommendation. */
export function QuestionCard({
  id,
  text,
  before,
  was,
  selected,
  decided,
  auto,
  locked,
  ...p
}: Props) {
  const changed = (now: unknown, then: unknown) => !!before && !same(now, then);
  const wasLabel = was?.option != null && before ? before.options[was.option]?.label : undefined;
  return (
    <article className={`qq-card${p.className ? ` ${p.className}` : ""}`}>
      <h2 className="qq-subject">
        <span className="qq-id">{id}</span>
        <span className={changed(text.subject, before?.subject) ? "qq-changed" : undefined}>
          <Inline>{text.subject}</Inline>
        </span>
      </h2>
      {p.deps}
      <section className="qq-context">
        <h4 className="qq-label">Context</h4>
        <ul>
          {text.context.map((c) => (
            <li key={c} className={changed(true, before?.context.includes(c)) ? "qq-changed" : ""}>
              <Inline>{c}</Inline>
            </li>
          ))}
        </ul>
        {p.sources}
      </section>
      <h4 className="qq-label">Options</h4>
      <ol className="qq-options">
        {text.options.map((o, i) => (
          <li key={o.label}>
            <div className="qq-option-row">
              <button
                type="button"
                className={[
                  "qq-option",
                  selected === i && "selected",
                  decided === i && (auto ? "auto" : "decided"),
                  changed(o, before?.options[i]) && "qq-changed",
                  (locked || !p.onSelect) && "readonly",
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
                    {i === 0 && <span className="pill rec">recommended</span>}
                    {decided === i &&
                      (auto ? (
                        <span className="pill auto">auto-answer</span>
                      ) : (
                        <span className="pill yours">your answer</span>
                      ))}
                    {wasLabel === o.label && (
                      <span className="pill was">
                        {was?.auto ? "auto-answer before" : "your answer before"}
                      </span>
                    )}
                  </span>
                  {o.means && (
                    <span className="qq-means">
                      <Inline>{o.means}</Inline>
                    </span>
                  )}
                  <ul className="qq-tradeoffs">
                    {TAGS.flatMap(([tag, key]) =>
                      (o[key] ?? []).map((b) => (
                        <li key={`${tag}${b}`}>
                          <span className={`qq-tag ${key}`}>{tag}</span>
                          <span>
                            <Inline>{b}</Inline>
                          </span>
                        </li>
                      )),
                    )}
                  </ul>
                  {!!o.bullets?.length && (
                    <ul>
                      {o.bullets.map((b) => (
                        <li key={b}>
                          <Inline>{b}</Inline>
                        </li>
                      ))}
                    </ul>
                  )}
                </span>
              </button>
              {p.onExplain && (
                <button
                  type="button"
                  className="qq-explain"
                  title={`Explain option ${i + 1}`}
                  aria-label={`Explain option ${i + 1}`}
                  onClick={() => p.onExplain?.(i)}
                >
                  <Icon name="info" size={15} />
                </button>
              )}
            </div>
            {p.under?.(i)}
          </li>
        ))}
      </ol>
      <p className={`qq-rec${changed(text.why, before?.why) ? " qq-changed" : ""}`}>
        <b>Recommended: option 1.</b> <Inline>{text.why}</Inline>
      </p>
    </article>
  );
}
