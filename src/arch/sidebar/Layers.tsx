import type { LayerDto } from "../../bindings";
import type { CurrentReview } from "../../review/current";
import type { WorkspaceName } from "../../workspaces/context";
import type { ArchSelection } from "../state";
import { layerDone } from "./logic";
import { Row, Sec, showInTab } from "./parts";

interface Props {
  workspace: WorkspaceName;
  review: CurrentReview;
  layers: LayerDto[];
  /** Assess wrote them, so they have titles rather than a component each. */
  assessed: boolean;
  selection: ArchSelection | undefined;
}

/** The change stack, each layer with its review progress. */
export function Layers({ workspace, review, layers, assessed, selection }: Props) {
  return (
    <>
      <Sec title="Layers" count={layers.length} />
      <ul className="arch-items">
        {layers.map((l) => (
          <li key={l.n}>
            <Row
              cur={selection?.kind === "layer" && selection.n === l.n}
              onClick={() => showInTab(workspace, review.id, { kind: "layer", n: l.n })}
            >
              <span className="arch-num">{l.n}</span>
              <span className={assessed ? undefined : "arch-what"}>{l.title}</span>
              <span />
              {l.summary && <span className="arch-why">{l.summary}</span>}
              <span className="arch-prog">
                <i style={{ width: `${(layerDone(review, l) ?? 0) * 100}%` }} />
              </span>
            </Row>
          </li>
        ))}
      </ul>
    </>
  );
}
