import { useEffect, useRef, useState } from "react";
import type { LookDto } from "../../bindings";
import type { WorkspaceName } from "../../core/workspace";
import type { CurrentReview } from "../../review/current";
import { onShowLook } from "../showLook";
import { checkLook } from "../state";
import { bySeverity } from "./logic";
import { Box, Fold, Row } from "./parts";

interface Props {
  workspace: WorkspaceName;
  review: CurrentReview;
  look: LookDto[];
  onShow(l: LookDto): void;
  onError(error: string | undefined): void;
}

/** How long the open items flash when the tab's count asks for them. */
const FLASH_MS = 1200;

/** What to scrutinize, worst first, each checked off by hand. */
export function Look({ workspace, review, look, onShow, onError }: Props) {
  const list = useRef<HTMLUListElement>(null);
  const [flash, setFlash] = useState(0);
  useEffect(
    () =>
      onShowLook(workspace, () => {
        setFlash((n) => n + 1);
        requestAnimationFrame(() =>
          list.current?.querySelector("li:not(.done)")?.scrollIntoView({ block: "nearest" }),
        );
      }),
    [workspace],
  );
  useEffect(() => {
    if (!flash) return;
    const t = setTimeout(() => setFlash(0), FLASH_MS);
    return () => clearTimeout(t);
  }, [flash]);

  async function check(l: LookDto) {
    onError(await checkLook(review, l.key, !l.checked));
  }
  if (!look.length) return null;
  return (
    <Fold title="Look" count={look.filter((l) => !l.checked).length} open={flash}>
      <ul ref={list} className={`arch-items${flash ? " flash" : ""}`}>
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
