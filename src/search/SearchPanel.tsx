import { useWorkspace } from "../core/workspace";
import { RefsView } from "./RefsView";
import { type SearchMode, setMode, useSearch } from "./store";
import { TextView } from "./TextView";
import "./search.css";

const MODES: [SearchMode, string][] = [
  ["references", "References"],
  ["text", "Text"],
];

/** The Search tab: references to a symbol, and text search across the workspace. */
export function SearchPanel() {
  const ws = useWorkspace();
  const s = useSearch(ws);
  return (
    <div className="sr-panel">
      <div className="sr-modes">
        {MODES.map(([m, label]) => (
          <button
            key={m}
            type="button"
            className={s.mode === m ? "on" : undefined}
            onClick={() => setMode(ws, m)}
          >
            {label}
          </button>
        ))}
      </div>
      {s.mode === "references" ? (
        <RefsView ws={ws} refs={s.refs} />
      ) : (
        <TextView ws={ws} text={s.text} />
      )}
    </div>
  );
}
