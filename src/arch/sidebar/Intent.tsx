import { useState } from "react";
import type { ComponentDto, IntentDto } from "../../bindings";
import { commands } from "../../ipc";
import type { CurrentReview } from "../../review/current";
import { useSendText } from "../../review/useSendText";
import { submitNextSend } from "../../terminal/registry";
import { contextMenu } from "../../ui/contextMenu";
import { IconButton } from "../../ui/icons";
import { PipeButton } from "../../ui/PipeButton";
import type { WorkspaceName } from "../../workspaces/context";
import { type ArchSelection, reloadArch } from "../state";
import { intentText, STATE_LABEL } from "./logic";
import { Box, Row, Sec, showInTab } from "./parts";

interface Props {
  workspace: WorkspaceName;
  review: CurrentReview;
  intents: IntentDto[];
  components: ComponentDto[];
  selection: ArchSelection | undefined;
  onError(error: string | undefined): void;
}

/** Your marks and whether the code meets them, picked to fix, send, or unmark. */
export function Intent({ workspace, review, intents, components, selection, onError }: Props) {
  const [checked, setChecked] = useState<Set<number>>(new Set());
  const { sendText, picker } = useSendText(workspace, onError, () => {});
  const picked = intents.filter((i) => checked.has(i.id)).map((i) => i.id);
  /** Tags and met marks have nothing to fix. */
  const fixable = intents
    .filter((i) => checked.has(i.id) && (i.state === "broken" || i.state === "open"))
    .map((i) => i.id);
  const names = new Map(components.map((c) => [c.id, c.name]));
  const name = (id: string) => names.get(id) ?? id;

  const toggle = (id: number) => {
    const next = new Set(checked);
    if (!next.delete(id)) next.add(id);
    setChecked(next);
  };
  const clear = () => setChecked(new Set());

  async function send(ids: number[]) {
    const r = await commands.intentPrompt(review.id, ids);
    if (r.status === "error") return onError(r.error);
    sendText(r.data, clear);
  }

  async function unmark(ids: number[]) {
    for (const id of ids) {
      const r = await commands.removeIntent(id);
      if (r.status === "error") onError(r.error);
    }
    clear();
    reloadArch(review.id);
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (!e.metaKey || !e.shiftKey || (e.key !== ">" && e.key !== ".")) return;
    if (picked.length === 0) return;
    e.preventDefault();
    send(picked);
  }

  const menu = (it: IntentDto) =>
    contextMenu(() => [
      {
        label: "Fix",
        disabled: it.state !== "broken" && it.state !== "open",
        onSelect: () => {
          submitNextSend();
          send([it.id]);
        },
      },
      {
        label: "Send to Terminal",
        keys: "⌘>",
        disabled: it.state === "tag",
        onSelect: () => send([it.id]),
      },
      "-",
      { label: "Unmark", onSelect: () => unmark([it.id]) },
    ]);

  return (
    <div onKeyDown={onKeyDown}>
      <Sec title="Intent" count={intents.length} />
      <ul className="arch-items">
        {intents.map((it) => {
          const [what, why] = intentText(it, name);
          return (
            <li key={it.id} className="arch-check" onContextMenu={menu(it)}>
              <Box on={checked.has(it.id)} label={what} onToggle={() => toggle(it.id)} />
              <Row
                cur={selection?.kind === "intent" && selection.id === it.id}
                onClick={() => showInTab(workspace, review.id, { kind: "intent", id: it.id })}
              >
                <span className="arch-what">{what}</span>
                <span className={`arch-st ${it.state}`}>{STATE_LABEL[it.state]}</span>
                <span className="arch-why">{why}</span>
              </Row>
            </li>
          );
        })}
      </ul>
      {picked.length > 0 && (
        <div className="arch-picked">
          <b>{picked.length} picked</b>
          <span className="spacer" />
          <button type="button" onClick={() => unmark(picked)}>
            Unmark
          </button>
          <PipeButton disabled={fixable.length === 0} onClick={() => send(fixable)}>
            Fix
          </PipeButton>
          <IconButton icon="x" label="Clear picks" onClick={clear} />
        </div>
      )}
      {picker}
    </div>
  );
}
