import { message } from "@tauri-apps/plugin-dialog";
import type { ArchDto, ComponentDto, EdgeDto, IntentKind, LensEdit } from "../bindings";
import { commands, errorText, unwrap } from "../ipc";
import type { CtxEntry } from "../ui/contextMenu";
import { promptText } from "../ui/prompt";
import { reloadArch } from "./state";

/** Runs an edit, then loads the architecture again. Failures show in a dialog. */
async function edit(review: number, call: () => Promise<unknown>) {
  try {
    await call();
    reloadArch(review);
  } catch (e) {
    message(errorText(e), { kind: "error" });
  }
}

const lens = (review: number, e: LensEdit) => edit(review, () => unwrap(commands.archLens(e)));
export const addIntent = (review: number, kind: IntentKind) =>
  edit(review, () => unwrap(commands.addIntent(kind)));
export const removeIntent = (review: number, id: number) =>
  edit(review, () => unwrap(commands.removeIntent(id)));

/** Asks for an optional reason, then marks `from` using `to` as wrong. */
export async function markWrong(review: number, from: string, to: string) {
  const reason = await promptText("Mark Wrong", { action: "Mark Wrong", allowEmpty: true });
  if (reason === undefined) return;
  await addIntent(review, { kind: "wrong", from, to, reason: reason || null });
}

/** The tag or note intent of `kind` on a component, if there is one. */
const tagOf = (arch: ArchDto, kind: "risk" | "trust" | "note", component: string) =>
  arch.intents.find((i) => i.kind === kind && i.component === component);

function toggleTag(review: number, arch: ArchDto, kind: "risk" | "trust", component: string) {
  const had = tagOf(arch, kind, component);
  return had ? removeIntent(review, had.id) : addIntent(review, { kind, component });
}

async function editNote(review: number, arch: ArchDto, c: ComponentDto) {
  const text = await promptText(`Note on ${c.name}`, {
    value: c.note ?? "",
    action: "Save",
    multiline: true,
    allowEmpty: true,
  });
  if (text === undefined || text === (c.note ?? "")) return;
  const had = tagOf(arch, "note", c.id);
  await edit(review, async () => {
    if (had) await unwrap(commands.removeIntent(had.id));
    if (text) await unwrap(commands.addIntent({ kind: "note", component: c.id, text }));
  });
}

async function rename(review: number, c: ComponentDto) {
  const name = await promptText(`Rename ${c.name}`, { value: c.name, action: "Rename" });
  if (name && name !== c.name) await lens(review, { kind: "rename", component: c.id, name });
}

/** A component's menu: lens edits, then tags and the note. */
export function componentItems(review: number, arch: ArchDto, c: ComponentDto): CtxEntry[] {
  const others = arch.components.filter((o) => o.id !== c.id);
  return [
    { label: "Rename…", onSelect: () => rename(review, c) },
    {
      label: "Merge Into",
      disabled: others.length === 0,
      items: others.map((o) => ({
        label: o.name,
        onSelect: () => lens(review, { kind: "merge", component: c.id, into: o.id }),
      })),
    },
    { label: "Split by Folder", onSelect: () => lens(review, { kind: "split", component: c.id }) },
    "-",
    { label: "Tag Risk", checked: c.risk, onSelect: () => toggleTag(review, arch, "risk", c.id) },
    {
      label: "Tag Trust Boundary",
      checked: c.trust,
      onSelect: () => toggleTag(review, arch, "trust", c.id),
    },
    { label: "Note…", onSelect: () => editNote(review, arch, c) },
  ];
}

/** What Send to Terminal pastes for an edge: its Wrong mark's prompt, or the edge and its
 * import sites. */
export async function edgeText(review: number, arch: ArchDto, e: EdgeDto): Promise<string> {
  if (e.wrong !== null) return unwrap(commands.intentPrompt(review, [e.wrong]));
  const name = (id: string) => arch.components.find((c) => c.id === id)?.name ?? id;
  const sites = e.imports.map((i) => `- ${i.fromPath}:${i.line} imports ${i.toPath}`);
  return [`${name(e.from)} uses ${name(e.to)}:`, ...sites].join("\n");
}

/** An edge's menu. Unmark replaces Mark Wrong… once it's marked. */
export function edgeItems(review: number, e: EdgeDto, send: () => void): CtxEntry[] {
  const wrong = e.wrong;
  return [
    wrong === null
      ? { label: "Mark Wrong…", keys: "w", onSelect: () => markWrong(review, e.from, e.to) }
      : { label: "Unmark", onSelect: () => removeIntent(review, wrong) },
    "-",
    { label: "Send to Terminal", keys: "⌘>", onSelect: send },
  ];
}
