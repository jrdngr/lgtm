import { useState } from "react";
import type { ComponentDto, IntentDto } from "../../bindings";
import { commands } from "../../ipc";
import type { CurrentReview } from "../../review/current";
import { useSendText } from "../../review/useSendText";
import { AgentButton } from "../../ui/AgentButton";
import { contextMenu } from "../../ui/contextMenu";
import { Icon, IconButton } from "../../ui/icons";
import type { WorkspaceName } from "../../workspaces/context";
import { fixIntents, stopIntentFix, useArchRun } from "../runs";
import { type ArchSelection, reloadArch } from "../state";
import { intentText, STATE_LABEL } from "./logic";
import { Box, Row, Sec, showInTab } from "./parts";
import "../../review/notes.css";
import { PipeButton } from "../../ui/PipeButton";

interface Props {
  workspace: WorkspaceName;
  review: CurrentReview;
  intents: IntentDto[];
  components: ComponentDto[];
  selection: ArchSelection | undefined;
  onError(error: string | undefined): void;
}

/** Your marks and whether the code meets them, picked to Fix, send, or unmark. */
export function Intent({ workspace, review, intents, components, selection, onError }: Props) {
  const [checked, setChecked] = useState<Set<number>>(new Set());
  const run = useArchRun("intentFix", review.id);
  const running = !!run?.running;
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

  function fix(ids: number[]) {
    fixIntents(workspace, review.id, review.revset, ids);
    clear();
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
        disabled: running || (it.state !== "broken" && it.state !== "open"),
        onSelect: () => fix([it.id]),
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
      {running && (
        <div className="note-run">
          <span className="note-spin" />
          <span className="note-run-text">{run?.note}</span>
          <button type="button" className="quiet" onClick={() => stopIntentFix(review.id)}>
            <Icon name="stop" size={12} />
            Stop
          </button>
        </div>
      )}
      {!running && run?.error && <div className="note-run error">{run.error}</div>}
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
          <PipeButton onClick={() => send(picked)}>Intents</PipeButton>
          <AgentButton disabled={running || fixable.length === 0} onClick={() => fix(fixable)}>
            Fix
          </AgentButton>
          <IconButton icon="x" label="Clear picks" onClick={clear} />
        </div>
      )}
      {picker}
    </div>
  );
}
