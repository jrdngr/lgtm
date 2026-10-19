import type { LayerDto } from "../../bindings";
import type { CurrentReview } from "../../review/current";
import { layerDone } from "./logic";
import { Fold, Row } from "./parts";

interface Props {
  review: CurrentReview;
  layers: LayerDto[];
  /** Assess wrote them, so they have titles rather than a component each. */
  assessed: boolean;
}

/** The change stack, each layer with its review progress. A layer opens at its first change. */
export function Layers({ review, layers, assessed }: Props) {
  if (!layers.length) return null;
  return (
    <Fold title="Layers" count={layers.length}>
      <ul className="arch-items">
        {layers.map((l) => {
          const first = l.ranges[0];
          return (
            <li key={l.n}>
              <Row cur={false} onClick={() => first && review.show(first.path, first.start)}>
                <span className="arch-num">{l.n}</span>
                <span className={assessed ? undefined : "arch-what"}>{l.title}</span>
                <span />
                {l.summary && <span className="arch-why">{l.summary}</span>}
                <span className="arch-prog">
                  <i style={{ width: `${(layerDone(review, l) ?? 0) * 100}%` }} />
                </span>
              </Row>
            </li>
          );
        })}
      </ul>
    </Fold>
  );
}
