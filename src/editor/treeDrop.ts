import type { DroppedFile } from "../bindings";
import { commands, unwrap } from "../ipc";
import { confirmAction } from "../ui/prompt";
import type { WorkspaceName } from "../workspaces/context";

/** A dropped file's contents as base64. */
function base64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).replace(/^data:[^,]*,/, ""));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

const fileOf = (e: FileSystemFileEntry) => new Promise<File>((ok, err) => e.file(ok, err));

/** Every entry in a dropped folder, read in batches as the browser hands them out. */
async function children(dir: FileSystemDirectoryEntry): Promise<FileSystemEntry[]> {
  const reader = dir.createReader();
  const out: FileSystemEntry[] = [];
  for (;;) {
    const batch = await new Promise<FileSystemEntry[]>((ok, err) => reader.readEntries(ok, err));
    if (!batch.length) return out;
    out.push(...batch);
  }
}

/** The files under a dropped entry, each with its path under `dir`. */
async function collect(entry: FileSystemEntry, dir: string): Promise<DroppedFile[]> {
  const path = dir ? `${dir}/${entry.name}` : entry.name;
  if (entry.isFile) {
    return [{ path, data: await base64(await fileOf(entry as FileSystemFileEntry)) }];
  }
  const inner = await children(entry as FileSystemDirectoryEntry);
  return (await Promise.all(inner.map((e) => collect(e, path)))).flat();
}

/** Copies the files and folders dropped in `data` into `dir`, asking before replacing any.
 * Returns the paths added at the top. */
export async function dropInto(
  ws: WorkspaceName,
  dir: string,
  data: DataTransfer,
): Promise<string[]> {
  // Entries go stale once the drop event returns, so take them all first.
  const entries = [...data.items].flatMap((i) => i.webkitGetAsEntry() ?? []);
  const files = (await Promise.all(entries.map((e) => collect(e, dir)))).flat();
  const taken = await unwrap(commands.addFiles(ws, files, false));
  if (taken.length) {
    const what = taken.length === 1 ? taken[0] : `${taken.length} files`;
    if (await confirmAction(`Replace ${what}?`, "Replace")) {
      const again = new Set(taken);
      await unwrap(
        commands.addFiles(
          ws,
          files.filter((f) => again.has(f.path)),
          true,
        ),
      );
    }
  }
  return entries.map((e) => (dir ? `${dir}/${e.name}` : e.name));
}
