import type { LookDto } from "../../bindings";
import { commands } from "../../ipc";
import type { WorkspaceName } from "../../workspaces/context";
import { type ArchSelection, reloadArch } from "../state";
import { bySeverity } from "./logic";
import { Box, Row, Sec, showInTab } from "./parts";

interface Props {
  workspace: WorkspaceName;
  review: number;
  look: LookDto[];
  selection: ArchSelection | undefined;
  onError(error: string | undefined): void;
}

/** What to scrutinize, worst first, each checked off by hand. */
export function Look({ workspace, review, look, selection, onError }: Props) {
  async function check(l: LookDto) {
    const r = await commands.checkLook(review, l.key, !l.checked);
    onError(r.status === "error" ? r.error : undefined);
    if (r.status === "ok") reloadArch(review);
  }
  return (
    <>
      <Sec title="Look" count={look.filter((l) => !l.checked).length} />
      <ul className="arch-items">
        {bySeverity(look).map((l) => (
          <li key={l.key} className={`arch-check${l.checked ? " done" : ""}`}>
            <Box on={l.checked} label={l.title} onToggle={() => check(l)} />
            <Row
              cur={selection?.kind === "look" && selection.key === l.key}
              onClick={() => showInTab(workspace, review, { kind: "look", key: l.key })}
            >
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
    </>
  );
}
