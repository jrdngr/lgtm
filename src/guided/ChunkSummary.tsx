import type { ReactNode } from "react";
import type { ChunkSummary as Summary } from "../bindings";
import { Markdown } from "../bots/Markdown";

/** A chunk's summary, one labeled section for each part the agent filled in. */
export function ChunkSummary({ summary: s = {} }: { summary?: Summary }) {
  const context = s.context ?? [];
  const decisions = s.decisions ?? [];
  const behavior = s.behavior ?? [];
  const iface = s.interface ?? [];
  const data = s.data ?? [];
  const lists = [context, decisions, behavior, iface, data];
  if (!s.purpose && lists.every((l) => !l.length)) return null;
  return (
    <div className="gd-summary">
      {s.purpose && (
        <Section label="Purpose">
          <Markdown>{s.purpose}</Markdown>
        </Section>
      )}
      {context.length > 0 && (
        <Section label="Context">
          {context.map((q) => (
            <div key={q.question} className="gd-qa">
              <div className="gd-q">{q.question}</div>
              <Markdown>{q.answer ?? ""}</Markdown>
            </div>
          ))}
        </Section>
      )}
      {decisions.length > 0 && (
        <Section label="Decisions">
          <Bullets items={decisions} />
        </Section>
      )}
      {behavior.length > 0 && (
        <Section label="Behavior">
          {behavior.map((b) => (
            <div key={`${b.before}|${b.after}`} className="gd-change">
              {b.before && (
                <div className="gd-before">
                  <Markdown>{b.before}</Markdown>
                </div>
              )}
              {b.before && b.after && <span className="gd-arrow">→</span>}
              {b.after && (
                <div className="gd-after">
                  <Markdown>{b.after}</Markdown>
                </div>
              )}
            </div>
          ))}
        </Section>
      )}
      {iface.length > 0 && (
        <Section label="Interface">
          {iface.map((i) => (
            <div key={`${i.name}|${i.before}|${i.after}`} className="gd-iface">
              {i.name && <div className="gd-iface-name">{i.name}</div>}
              <pre>
                {lines(i.before).map((l, n) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: a signature's lines never reorder
                  <div key={`b${n}`} className="since-removed">
                    − {l}
                  </div>
                ))}
                {lines(i.after).map((l, n) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: a signature's lines never reorder
                  <div key={`a${n}`} className={i.before ? "since-added" : undefined}>
                    {i.before ? "+ " : ""}
                    {l}
                  </div>
                ))}
              </pre>
            </div>
          ))}
        </Section>
      )}
      {data.length > 0 && (
        <Section label="Data">
          <Bullets items={data} />
        </Section>
      )}
    </div>
  );
}

const lines = (code?: string) => (code ? code.split("\n") : []);

function Section({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <div className="gd-sum-label">{label}</div>
      <div className="gd-sum-body">{children}</div>
    </>
  );
}

function Bullets({ items }: { items: string[] }) {
  return (
    <ul className="gd-bullets">
      {items.map((d) => (
        <li key={d}>
          <Markdown>{d}</Markdown>
        </li>
      ))}
    </ul>
  );
}
