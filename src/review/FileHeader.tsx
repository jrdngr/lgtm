import type { CheckState, FileDto } from "../bindings";
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
  collapsed: boolean;
  onClick(): void;
  onToggle(): void;
  onCheck(checked: boolean): void;
}

/** Checkbox, collapse button, path, and tags. The checkbox gets a ring once every changed
 * line is reviewed, but is never checked automatically. */
export function FileHeader({ dto, check, progress, collapsed, onClick, onToggle, onCheck }: Props) {
  const checked = check === "checked";
  const allReviewed = !checked && progress.changed > 0 && progress.reviewed === progress.changed;
  const stop = (e: React.MouseEvent) => e.stopPropagation();
  return (
    <div
      className={`file-header${check === "changedSinceReviewed" ? " changed" : ""}`}
      onMouseDown={onClick}
    >
      <button type="button" className="collapse" onMouseDown={stop} onClick={onToggle}>
        {collapsed ? "▸" : "▾"}
      </button>
      <input
        type="checkbox"
        className={allReviewed ? "ring" : undefined}
        checked={checked}
        title={checked ? "Reviewed. Uncheck to restore line marks" : "Mark file reviewed"}
        onMouseDown={stop}
        onChange={() => onCheck(!checked)}
      />
      <span className={`status status-${dto.status}`}>{STATUS_LABEL[dto.status]}</span>
      <span className="path">
        {dto.oldPath && <span className="dir">{dto.oldPath} → </span>}
        <Path path={dto.path} />
      </span>
      {check === "changedSinceReviewed" && (
        <span className="tag amber">changed since reviewed</span>
      )}
      {dto.skipped && <span className="tag">skipped</span>}
      {dto.oldExec !== dto.newExec && <span className="tag">{dto.newExec ? "+x" : "-x"}</span>}
      {dto.binary && <span className="tag">binary</span>}
      {dto.conflict && <span className="tag conflict">conflict</span>}
      <span className="spacer" />
      {progress.flagged > 0 && (
        <span className="pill red" title={`${progress.flagged} flagged`}>
          ⚑ {progress.flagged}
        </span>
      )}
      <span className="file-count">
        {progress.reviewed}/{progress.changed}
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
