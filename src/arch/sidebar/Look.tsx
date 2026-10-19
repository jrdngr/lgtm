import type { LookDto } from "../../bindings";
import { commands } from "../../ipc";
import { reloadArch } from "../state";
import { bySeverity } from "./logic";
import { Box, Fold, Row } from "./parts";

interface Props {
  review: number;
  look: LookDto[];
  onShow(l: LookDto): void;
  onError(error: string | undefined): void;
}

/** What to scrutinize, worst first, each checked off by hand. */
export function Look({ review, look, onShow, onError }: Props) {
  async function check(l: LookDto) {
    const r = await commands.checkLook(review, l.key, !l.checked);
    onError(r.status === "error" ? r.error : undefined);
    if (r.status === "ok") reloadArch(review);
  }
  if (!look.length) return null;
  return (
    <Fold title="Look" count={look.filter((l) => !l.checked).length}>
      <ul className="arch-items">
        {bySeverity(look).map((l) => (
          <li key={l.key} className={`arch-check${l.checked ? " done" : ""}`}>
            <Box on={l.checked} label={l.title} onToggle={() => check(l)} />
            <Row cur={false} onClick={() => onShow(l)}>
              <span className="arch-what">{l.title}</span>
              <span className={`arch-sev ${l.severity}`}>{l.severity}</span>
              <span className="arch-why">
                {l.detail && `${l.detail} `}
                <span className="arch-src">· {l.source}</span>
              </span>
            </Row>
          </li>
        ))}
      </ul>
    </Fold>
  );
}
