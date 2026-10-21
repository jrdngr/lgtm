import { useEffect, useState } from "react";
import type { WorkspaceName } from "../core/workspace";
import { commands } from "../ipc";
import { IconButton } from "../ui/icons";
import { focusLog } from "./loadReview";
import { useLogRevset } from "./logRevset";

interface Props {
  workspace: WorkspaceName;
  editing: boolean;
  setEditing(editing: boolean): void;
}

/** The log's revset under the Revisions header: an input while editing, where Enter applies
 * it, else the revset when it isn't jj's default. */
export function RevsetStrip({ workspace, editing, setEditing }: Props) {
  const log = useLogRevset(workspace);
  const revset = log?.revset ?? null;
  /** jj's error for the revset last entered, which the log doesn't show. */
  const [refused, setRefused] = useState<string>();
  const error = refused ?? log?.error;
  useEffect(() => {
    if (!editing) setRefused(undefined);
  }, [editing]);

  async function apply(text: string) {
    const r = await commands.setLogRevset(workspace, text);
    setRefused(r.status === "error" ? r.error : undefined);
  }

  function close() {
    setEditing(false);
    focusLog();
  }

  if (!editing && revset === null) return null;
  return (
    <div className="revset-strip" onContextMenu={(e) => e.stopPropagation()}>
      <div className="revset-row">
        {editing ? (
          <input
            // The revset an agent sets replaces what's typed.
            key={revset ?? ""}
            className="revset-input"
            aria-label="Revset"
            // biome-ignore lint/a11y/noAutofocus: the strip opens to take a revset
            autoFocus
            spellCheck={false}
            defaultValue={revset ?? ""}
            onKeyDown={(e) => {
              if (e.key === "Enter") apply(e.currentTarget.value);
              else if (e.key === "Escape") close();
            }}
          />
        ) : (
          <button type="button" className="revset-text" onClick={() => setEditing(true)}>
            {revset}
          </button>
        )}
        {editing ? (
          <IconButton icon="close" label="Close" onClick={close} />
        ) : (
          <IconButton icon="retry" label="Reset" onClick={() => apply("")} />
        )}
      </div>
      {error && <div className="revset-error">{error}</div>}
    </div>
  );
}
