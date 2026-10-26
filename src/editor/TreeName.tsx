import { useState } from "react";
import { errorText } from "../ipc";

/** A new entry being named, or an entry being renamed, in the file tree. */
export type Naming =
  | { kind: "new"; dir: string; folder: boolean }
  | { kind: "rename"; path: string; folder: boolean };

/** A tree row whose name is a text field. Enter or leaving it with a name calls `onDone`,
 * which throws to keep the field open with the error; Escape or leaving it empty cancels. */
export function TreeName({
  initial,
  folder,
  pad,
  onDone,
  onCancel,
}: {
  initial: string;
  folder: boolean;
  pad: React.CSSProperties;
  onDone(name: string): Promise<void>;
  onCancel(): void;
}) {
  const [name, setName] = useState(initial);
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);

  async function done(leaving: boolean) {
    const n = name.trim();
    if (busy) return;
    if (!n || n === initial) return onCancel();
    setBusy(true);
    try {
      await onDone(n);
    } catch (e) {
      if (leaving) return onCancel();
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="tree-name" style={pad} onContextMenu={(e) => e.stopPropagation()}>
      <span className="chevron">{folder ? "▸" : ""}</span>
      <div className="tree-name-field">
        <input
          // biome-ignore lint/a11y/noAutofocus: the next thing typed is the name
          autoFocus
          aria-label="Name"
          value={name}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          className={error ? "invalid" : undefined}
          onFocus={(e) => {
            // Selects the name without its extension, as VS Code does.
            const dot = folder ? -1 : initial.lastIndexOf(".");
            e.currentTarget.setSelectionRange(0, dot > 0 ? dot : initial.length);
          }}
          onChange={(e) => {
            setName(e.target.value);
            setError(undefined);
          }}
          onBlur={() => done(true)}
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.key === "Enter") done(false);
            else if (e.key === "Escape") onCancel();
          }}
        />
        {error && <div className="tree-name-error">{error}</div>}
      </div>
    </div>
  );
}
