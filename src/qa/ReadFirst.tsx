import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { type WorkspaceName, workspaceRoot } from "../core/workspace";
import { commands } from "../ipc";
import { Icon } from "../ui/icons";
import { addPaths, repoRelative, splitPath } from "./readFirstPaths";

interface Props {
  ws: WorkspaceName;
  paths: string[];
  locked: boolean;
  onChange(paths: string[]): void;
}

/** Files the agent reads first: a drop area for files and folders from the Files tree or
 * Finder, rows to remove them, and Add for a file picker. */
export function ReadFirst({ ws, paths, locked, onChange }: Props) {
  const [over, setOver] = useState(false);
  const root = workspaceRoot(ws);
  const add = (picked: string[]) =>
    onChange(
      addPaths(
        paths,
        picked.map((p) => repoRelative(p, root)),
      ),
    );
  const files = (e: React.DragEvent) => !locked && e.dataTransfer.types.includes("Files");

  async function pick() {
    const picked = await open({ multiple: true, defaultPath: root });
    if (picked) add(picked);
  }

  return (
    <div
      className={`qa-read${over ? " over" : ""}${paths.length ? "" : " empty"}`}
      onDragOver={(e) => {
        if (!files(e)) return;
        e.preventDefault();
        e.dataTransfer.dropEffect = "copy";
        setOver(true);
      }}
      onDragLeave={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOver(false);
      }}
      onDrop={(e) => {
        if (!files(e)) return;
        e.preventDefault();
        setOver(false);
        commands.droppedPaths().then(add);
      }}
    >
      {paths.map((p) => {
        const { name, dir, folder } = splitPath(p);
        return (
          <div key={p} className="qa-read-row">
            <span className={`qa-read-icon${folder ? " folder" : ""}`}>
              <Icon name={folder ? "folder" : "file"} size={14} />
            </span>
            <span className="qa-read-name">{name}</span>
            <span className="qa-read-dir">{dir}</span>
            {!locked && (
              <button
                type="button"
                className="qa-read-remove"
                aria-label={`Remove ${p}`}
                onClick={() => onChange(paths.filter((x) => x !== p))}
              >
                ×
              </button>
            )}
          </div>
        );
      })}
      {!locked && (
        <div className="qa-read-add">
          <button type="button" className="qa-read-btn" onClick={pick}>
            <Icon name="plus" size={13} />
            Add
          </button>
        </div>
      )}
    </div>
  );
}
