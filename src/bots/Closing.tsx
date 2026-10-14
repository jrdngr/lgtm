import type { Report } from "./findings";
import { Markdown } from "./Markdown";

/** Links to findings by id, which scroll to their cards. */
export function Refs({ ids, onGo }: { ids: string[]; onGo(id: string): void }) {
  return (
    <>
      {ids.map((id) => (
        <button type="button" className="finding-ref" key={id} onClick={() => onGo(id)}>
          {id.includes("#") ? id : `#${id}`}
        </button>
      ))}
    </>
  );
}

/** Fix first, quick wins and questions under the synthesis. */
export function Closing({ r, onGo }: { r: Report; onGo(id: string): void }) {
  const s = r.synthesis;
  if (!s) return null;
  return (
    <>
      {s.fixFirst.length > 0 && (
        <section>
          <h3>Fix first</h3>
          <ol>
            {s.fixFirst.map((a) => (
              <li key={a.action}>
                <Markdown>{a.action}</Markdown> <Refs ids={a.closes} onGo={onGo} />
              </li>
            ))}
          </ol>
        </section>
      )}
      {s.quickWins.length > 0 && (
        <section>
          <h3>Quick wins</h3>
          <ul>
            {s.quickWins.map((q) => (
              <li key={q.finding}>
                <Refs ids={[q.finding]} onGo={onGo} /> <Markdown>{q.fix}</Markdown>
              </li>
            ))}
          </ul>
        </section>
      )}
      {s.questions.length > 0 && (
        <section>
          <h3>Questions for you</h3>
          <ol>
            {s.questions.map((q) => (
              <li key={q.question}>
                <Markdown>{q.question}</Markdown> <Refs ids={q.findings} onGo={onGo} />
              </li>
            ))}
          </ol>
        </section>
      )}
    </>
  );
}
