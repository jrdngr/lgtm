import { useRef, useState } from "react";
import { commands, unwrap } from "../ipc";
import { run } from "../ui/pathMenu";
import { foldersOf, moveInto } from "./treeActions";
import { dropInto } from "./treeDrop";

/** The folder a drag over `target` lands in: a folder row's own, a file row's folder, or the
 * root. */
function dropTarget(target: EventTarget): string {
  const row = (target as HTMLElement).closest<HTMLElement>("[data-path]");
  if (!row) return "";
  const path = row.dataset.path ?? "";
  return row.dataset.dir !== undefined ? path : (foldersOf(path).at(-1) ?? "");
}

/** Dragging rows out as files, and dropping files in: from Finder they're copied, and a row
 * dragged from the tree itself moves. */
export function useTreeDrag(workspace: string | null, madeAt: (p: string) => void) {
  const [dropDir, setDropDir] = useState<string>();
  /** The row being dragged out of the tree, until its drag ends. */
  const dragging = useRef<string | undefined>(undefined);

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
      dragging.current = path;
      unwrap(commands.dragOut(workspace, [path]))
        .catch(() => {})
        .finally(() => {
          dragging.current = undefined;
        });
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
      e.dataTransfer.dropEffect = dragging.current ? "move" : "copy";
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
      const from = dragging.current;
      if (from) return moveInto(workspace, from, dir, madeAt);
      run(dropInto(workspace, dir, e.dataTransfer).then((added) => added[0] && madeAt(added[0])));
    },
  };
  return { dropDir, dragProps };
}
