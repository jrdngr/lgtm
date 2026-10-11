import { message } from "@tauri-apps/plugin-dialog";

/** Save functions of editor panels with unsaved edits, by panel id. Each resolves to whether
 * the save worked. */
const unsaved = new Map<string, () => Promise<boolean>>();

export function setUnsaved(id: string, save: (() => Promise<boolean>) | undefined) {
  if (save) unsaved.set(id, save);
  else unsaved.delete(id);
}

/** Whether panel `id` may close: true when it has no unsaved edits, or the user saved or
 * discarded them. */
export async function confirmClose(id: string, name: string): Promise<boolean> {
  const save = unsaved.get(id);
  if (!save) return true;
  const choice = await message(`${name} has unsaved changes.`, {
    title: "Save changes?",
    kind: "warning",
    buttons: { yes: "Save", no: "Don't Save", cancel: "Cancel" },
  });
  if (choice === "Save") return save();
  return choice === "Don't Save";
}
