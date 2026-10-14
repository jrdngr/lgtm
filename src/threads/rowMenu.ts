import type { Stage, ThreadDto } from "../bindings";
import { type CtxEntry, copy } from "../ui/contextMenu";
import { openUrl } from "../ui/pathMenu";
import { STAGES } from "./stages";

interface Deps {
  stage(t: ThreadDto, s: Stage): void;
  /** Focuses the thread's reply box, or its agent box with `ask`, once it's open. */
  focusBox(ask: boolean): void;
  showCode(t: ThreadDto): void;
}

/** The right-click menu of a row in the Threads tab, with the keys the list binds. */
export function rowMenu(t: ThreadDto, d: Deps): CtxEntry[] {
  return [
    { label: "To Fix", keys: "f", onSelect: () => d.stage(t, "toFix") },
    {
      label: "Reply",
      keys: "r",
      onSelect: () => {
        d.stage(t, "reply");
        d.focusBox(true);
      },
    },
    { label: "Edit Reply", keys: "e", onSelect: () => d.focusBox(false) },
    { label: "Mark Final", keys: "y", onSelect: () => d.stage(t, "ready") },
    { label: "Done", keys: "d", onSelect: () => d.stage(t, "done") },
    "-",
    {
      label: "Move to Stage",
      items: STAGES.map(([s, label]) => ({
        label,
        checked: s === t.state.stage,
        onSelect: () => d.stage(t, s),
      })),
    },
    { label: "Show Code", keys: "o", disabled: !t.placed, onSelect: () => d.showCode(t) },
    t.github && { label: "Open on GitHub", onSelect: () => t.github && openUrl(t.github.url) },
    { label: "Copy Reply", disabled: !t.body, onSelect: () => copy(t.body) },
  ];
}
