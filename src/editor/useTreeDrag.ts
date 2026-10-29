import { useRef, useState } from "react";
import { commands, unwrap } from "../ipc";
import { run } from "../services/pathMenu";
import { copyInto, foldersOf, moveInto } from "./treeActions";
import { dropInto } from "./treeDrop";

/** The folder a drag over `target` lands in: a folder row's own, a file row's folder, or the
 * root. */
function dropTarget(target: EventTarget): string {
  const row = (target as HTMLElement).closest<HTMLElement>("[data-path]");
  if (!row) return "";
  const path = row.dataset.path ?? "";
  return row.dataset.dir !== undefined ? path : (foldersOf(path).at(-1) ?? "");
}

/** How long a row's drag stays known after macOS says it ended: WebKit can hand the page its
 * drop after that. */
const DROP_GRACE_MS = 1000;

/** Dragging rows out as files, and dropping files in: from Finder they're copied, and a row
 * dragged from the tree itself moves, or copies with Option held. */
export function useTreeDrag(workspace: string | null, madeAt: (p: string) => void) {
  const [dropDir, setDropDir] = useState<string>();
  /** The row being dragged out of the tree, until its drag ends. */
  const dragging = useRef<{ path: string } | undefined>(undefined);

  function onMouseDown(e: React.MouseEvent) {
    const row = (e.target as HTMLElement).closest<HTMLElement>("[data-path]");
    const path = row?.dataset.path;
    if (e.button !== 0 || !path) return;
    const [x, y] = [e.clientX, e.clientY];
    const stop = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", stop);
    };
    const move = (m: MouseEvent) => {
      if (Math.hypot(m.clientX - x, m.clientY - y) < 4) return;
      stop();
      const drag = { path };
      dragging.current = drag;
      unwrap(commands.dragOut(workspace, [path]))
        .catch(() => {})
        .finally(() =>
          setTimeout(() => {
            if (dragging.current === drag) dragging.current = undefined;
          }, DROP_GRACE_MS),
        );
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", stop);
  }

  const files = (e: React.DragEvent) => e.dataTransfer.types.includes("Files");
  const dragProps = {
    onMouseDown,
    onDragOver(e: React.DragEvent) {
      if (!files(e)) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = dragging.current && !e.altKey ? "move" : "copy";
      setDropDir(dropTarget(e.target));
    },
    onDragLeave(e: React.DragEvent) {
      if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDropDir(undefined);
    },
    onDrop(e: React.DragEvent) {
      if (!files(e)) return;
      e.preventDefault();
      setDropDir(undefined);
      const dir = dropTarget(e.target);
      const from = dragging.current?.path;
      dragging.current = undefined;
      if (from) return (e.altKey ? copyInto : moveInto)(workspace, from, dir, madeAt);
      run(dropInto(workspace, dir, e.dataTransfer).then((added) => added[0] && madeAt(added[0])));
    },
  };
  return { dropDir, dragProps };
}
