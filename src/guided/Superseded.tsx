import { useState } from "react";
import type { ContextDocDto } from "../bindings";
import type { WorkspaceName } from "../core/workspace";
import { openFile } from "../editor/open";
import { Lifted } from "../ui/Lifted";

/** Superseded ADRs folded into one button, listing each by number and title to open. */
export function Superseded({
  workspace,
  docs,
  right,
}: {
  workspace: WorkspaceName;
  docs: ContextDocDto[];
  right?: boolean;
}) {
  const [open, setOpen] = useState(false);
  if (!docs.length) return null;
  return (
    <span className="gd-pop-anchor">
      <button type="button" className="gd-btn" aria-expanded={open} onClick={() => setOpen(!open)}>
        Superseded Decisions {docs.length}
      </button>
      {open && (
        <Lifted className={`gd-pop${right ? " right" : ""}`}>
          {docs.map((d) => (
            <button
              key={d.path}
              type="button"
              className="gd-mi"
              onClick={() => {
                setOpen(false);
                openFile(workspace, d.path);
              }}
            >
              {d.number && <b className="mono">{d.number}</b>}
              <span>{d.title}</span>
            </button>
          ))}
        </Lifted>
      )}
    </span>
  );
}
