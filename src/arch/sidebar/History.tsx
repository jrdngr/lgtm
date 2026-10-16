import { useEffect, useState } from "react";
import type { HistoryDto } from "../../bindings";
import { commands } from "../../ipc";
import type { CurrentReview } from "../../review/current";
import { IconButton } from "../../ui/icons";
import { type ArchView, setArchView } from "../state";
import { clickRange, pickedCommits } from "./logic";
import { Row, Sec } from "./parts";

const count = (n: number, one: string) => `${n} ${n === 1 ? one : `${one}s`}`;

/** A commit's architecture delta, additions first. */
function delta(h: HistoryDto): [string, string][] {
  const out: [string, string][] = [];
  if (h.componentsAdded) out.push(["g", `+${count(h.componentsAdded, "component")}`]);
  if (h.componentsRemoved) out.push(["r", `−${count(h.componentsRemoved, "component")}`]);
  if (h.edgesAdded) out.push(["g", `+${count(h.edgesAdded, "edge")}`]);
  if (h.edgesRemoved) out.push(["r", `−${count(h.edgesRemoved, "edge")}`]);
  return out;
}

/** The review's commits that changed the architecture. Click one for its delta, shift-click a
 * second to compare them, click the shown one again for the whole review. */
export function History({ review, view }: { review: CurrentReview; view: ArchView }) {
  const [list, setList] = useState<HistoryDto[]>([]);
  const { id, files } = review;
  // biome-ignore lint/correctness/useExhaustiveDependencies: a new diff means new commits.
  useEffect(() => {
    let live = true;
    commands.archHistory(id).then((r) => live && setList(r.status === "ok" ? r.data : []));
    return () => {
      live = false;
    };
  }, [id, files]);
  if (list.length === 0) return null;
  const picked = pickedCommits(list, view.range);
  return (
    <>
      <Sec title="History" count={list.length}>
        {view.range && (
          <IconButton
            icon="x"
            label="Whole review"
            onClick={() => setArchView(id, { range: undefined })}
          />
        )}
      </Sec>
      <ul className="arch-items arch-hist">
        {list.map((h) => (
          <li key={h.commitId}>
            <Row
              cur={picked.includes(h.commitId)}
              onClick={(e) =>
                setArchView(id, { range: clickRange(list, view.range, h, e.shiftKey) })
              }
            >
              <span className="arch-id">{h.changeId.slice(0, 3)}</span>
              <span className="arch-desc">{h.description.split("\n")[0] || "no description"}</span>
              <span className="arch-delta">
                {delta(h).map(([tone, text]) => (
                  <span key={text} className={tone}>
                    {text}
                  </span>
                ))}
              </span>
            </Row>
          </li>
        ))}
      </ul>
    </>
  );
}
