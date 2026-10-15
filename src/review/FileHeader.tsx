import type { CheckState, FileDto } from "../bindings";
import { IconButton } from "../ui/icons";
import { MarkCounts } from "./MarkCounts";
import type { MarkSetup } from "./marks";
import type { Progress } from "./state";
import "./FileHeader.css";

export const STATUS_LABEL = {
  modified: "M",
  added: "A",
  removed: "D",
  copied: "C",
  renamed: "R",
} as const;

interface Props {
  dto: FileDto;
  check: CheckState;
  progress: Progress;
  setup: MarkSetup;
  collapsed: boolean;
  onClick(): void;
  onToggle(): void;
  onCheck(checked: boolean): void;
  /** Clears the file's marks and unchecks it. */
  onClear(): void;
  onContextMenu(e: React.MouseEvent): void;
  /** Whether amber rows show. */
  amber: boolean;
  onToggleAmber(): void;
}

/** Checkbox, collapse button, path, tags, and marks by color. The checkbox gets a ring once
 * every changed line is done, but is never checked automatically. */
export function FileHeader(props: Props) {
  const { dto, check, progress, collapsed, onClick, onToggle, onCheck } = props;
  const checked = check === "checked";
  const allDone = !checked && progress.changed > 0 && progress.done === progress.changed;
  const marked = Object.values(progress.marks).some((n) => n > 0);
  const stop = (e: React.MouseEvent) => e.stopPropagation();
  return (
    <div
      className={`file-header${check === "changedSinceReviewed" ? " changed" : ""}`}
      onMouseDown={onClick}
      onContextMenu={props.onContextMenu}
    >
      <button type="button" className="collapse" onMouseDown={stop} onClick={onToggle}>
        {collapsed ? "▸" : "▾"}
      </button>
      <input
        type="checkbox"
        className={allDone ? "ring" : undefined}
        checked={checked}
        title={checked ? "Done. Uncheck to restore line marks" : "Mark file done"}
        onMouseDown={stop}
        onChange={() => onCheck(!checked)}
      />
      <span className={`status status-${dto.status}`}>{STATUS_LABEL[dto.status]}</span>
      <span className="path">
        {dto.oldPath && <span className="dir">{dto.oldPath} → </span>}
        <Path path={dto.path} />
      </span>
      {check === "changedSinceReviewed" && (
        <button
          type="button"
          className={`tag amber${props.amber ? " on" : ""}`}
          aria-pressed={props.amber}
          onMouseDown={stop}
          onClick={props.onToggleAmber}
        >
          changed since reviewed
        </button>
      )}
      {dto.skipped && <span className="tag">skipped</span>}
      {dto.oldExec !== dto.newExec && <span className="tag">{dto.newExec ? "+x" : "-x"}</span>}
      {dto.binary && <span className="tag">binary</span>}
      {dto.conflict && <span className="tag conflict">conflict</span>}
      <span className="spacer" />
      <IconButton
        icon="eraser"
        label="Clear marks in this file"
        disabled={!marked && !checked}
        onMouseDown={stop}
        onClick={props.onClear}
      />
      <MarkCounts progress={progress} setup={props.setup} />
      <span className="file-count" title={`${progress.done} of ${progress.changed} done`}>
        {progress.done}/{progress.changed}
      </span>
    </div>
  );
}

/** A path with its directory dimmed so the filename stands out. */
function Path({ path }: { path: string }) {
  const cut = path.lastIndexOf("/") + 1;
  return (
    <>
      {cut > 0 && <span className="dir">{path.slice(0, cut)}</span>}
      {path.slice(cut)}
    </>
  );
}
