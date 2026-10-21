import { useState } from "react";
import type { NoteDto, NoteLevel, NotesDto } from "../bindings";
import { dockOf } from "../core/docks";
import { openNote } from "../core/panels";
import { loadState, saveState } from "../core/savedState";
import { useWorkspace, type WorkspaceName } from "../core/workspace";
import { commands, errorText, unwrap } from "../ipc";
import { guideAt, indent, toggled } from "../review/tree";
import { type CtxEntry, contextMenu } from "../ui/contextMenu";
import { Icon, IconButton } from "../ui/icons";
import { confirmAction, promptText } from "../ui/prompt";
import { ago } from "../ui/time";
import { QuickNote } from "./QuickNote";
import { useNotes } from "./store";
import { levelTree, type NoteFolder, parentOf } from "./tree";
import "../review/FilesPanel.css";
import "./notes.css";

const LEVELS: { level: NoteLevel; title: string }[] = [
  { level: "global", title: "Global" },
  { level: "repo", title: "Repo" },
  { level: "workspace", title: "Workspace" },
];

/** A dragged note's id. */
const DRAG = "application/x-lgtm-note";

const strings = (v: unknown): v is string[] =>
  Array.isArray(v) && v.every((s) => typeof s === "string");

/** What's open: a level by its name, a folder as `level:path`. Workspace starts open. */
function useOpen() {
  const [open, setOpen] = useState<ReadonlySet<string>>(
    () => new Set(loadState("notesOpen", ["workspace"], strings)),
  );
  const update = (next: Set<string>) => {
    setOpen(next);
    saveState("notesOpen", [...next]);
  };
  return [open, update] as const;
}

/** Notes at three levels, global, the repo's and the workspace's, each with folders, under a
 * quick note box. */
export function NotesPanel() {
  const workspace = useWorkspace();
  const { data, error: loadError } = useNotes(workspace);
  const [open, setOpen] = useOpen();
  const [error, setError] = useState<string>();
  const [drop, setDrop] = useState<string>();

  const act = (f: () => Promise<unknown>) =>
    f().then(
      () => setError(undefined),
      (e) => setError(errorText(e)),
    );
  const show = (n: NoteDto, edit = false) => {
    const api = dockOf(workspace);
    if (api) openNote(api, n.id, n.name, edit);
  };
  const reveal = (level: NoteLevel, folder: string) => {
    const next = new Set([...open, level]);
    for (let f = folder; f; f = parentOf(f)) next.add(`${level}:${f}`);
    setOpen(next);
  };

  const newNote = async (level: NoteLevel, folder: string) => {
    const name = await promptText("New Note", { action: "Create" });
    if (!name?.trim()) return;
    act(async () => {
      const n = await unwrap(commands.createNote(workspace, level, folder, name.trim(), ""));
      reveal(level, folder);
      show(n, true);
    });
  };
  const newFolder = async (level: NoteLevel, parent: string) => {
    const name = await promptText("New Folder", { action: "Create" });
    if (!name?.trim()) return;
    act(async () => {
      reveal(level, await unwrap(commands.createNoteFolder(workspace, level, parent, name.trim())));
    });
  };
  const renameNote = async (n: NoteDto) => {
    const name = await promptText("Rename Note", { value: n.name, action: "Rename" });
    if (name?.trim()) act(() => unwrap(commands.renameNote(workspace, n.id, name.trim())));
  };
  const deleteNote = async (n: NoteDto) => {
    if (!(await confirmAction(`Delete “${n.name}”?`, "Delete"))) return;
    dockOf(workspace)?.getPanel(`note-${n.id}`)?.api.close();
    act(() => unwrap(commands.deleteNote(workspace, n.id)));
  };
  const renameFolder = async (level: NoteLevel, f: NoteFolder) => {
    const name = await promptText("Rename Folder", { value: f.name, action: "Rename" });
    if (name?.trim()) act(() => unwrap(commands.renameNoteFolder(workspace, level, f.path, name)));
  };
  const deleteFolder = async (level: NoteLevel, f: NoteFolder) => {
    if (!(await confirmAction(`Delete “${f.name}” and its notes?`, "Delete"))) return;
    closeIn(workspace, f);
    act(() => unwrap(commands.deleteNoteFolder(workspace, level, f.path)));
  };

  /** Drag props that drop a note into `folder` of `level`. */
  const dropInto = (level: NoteLevel, folder: string) => {
    const key = `${level}:${folder}`;
    return {
      onDragOver(e: React.DragEvent) {
        if (!e.dataTransfer.types.includes(DRAG)) return;
        e.preventDefault();
        e.stopPropagation();
        e.dataTransfer.dropEffect = "move";
        setDrop(key);
      },
      onDragLeave(e: React.DragEvent) {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDrop(undefined);
      },
      onDrop(e: React.DragEvent) {
        const id = e.dataTransfer.getData(DRAG);
        if (!id) return;
        e.preventDefault();
        e.stopPropagation();
        setDrop(undefined);
        act(async () => {
          await unwrap(commands.moveNote(workspace, id, level, folder));
          reveal(level, folder);
        });
      },
    };
  };

  const tree = {
    open,
    drop,
    toggle: (key: string) => setOpen(toggled(open, key)),
    show,
    dropInto,
    noteMenu: (n: NoteDto): CtxEntry[] => [
      { label: "Open", onSelect: () => show(n) },
      { label: "Rename…", onSelect: () => renameNote(n) },
      "-",
      { label: "Delete…", onSelect: () => deleteNote(n) },
    ],
    folderMenu: (level: NoteLevel, f: NoteFolder): CtxEntry[] => [
      { label: "New Note…", onSelect: () => newNote(level, f.path) },
      { label: "New Folder…", onSelect: () => newFolder(level, f.path) },
      "-",
      { label: "Rename…", onSelect: () => renameFolder(level, f) },
      { label: "Delete…", onSelect: () => deleteFolder(level, f) },
    ],
  };

  return (
    <div className="notes-panel">
      <div className="notes-head">
        <b>Notes</b>
      </div>
      <QuickNote workspace={workspace} />
      <div className="notes-tree file-tree">
        {LEVELS.map(({ level, title }) => (
          <Level
            key={level}
            level={level}
            title={title}
            data={data}
            tree={tree}
            onNewNote={() => newNote(level, "")}
            onNewFolder={() => newFolder(level, "")}
          />
        ))}
      </div>
      {(error || loadError) && <div className="review-error">{error || loadError}</div>}
    </div>
  );
}

/** Closes the tabs of the notes in `f`, which is going away. */
function closeIn(workspace: WorkspaceName, f: NoteFolder) {
  for (const n of f.notes) dockOf(workspace)?.getPanel(`note-${n.id}`)?.api.close();
  for (const sub of f.folders) closeIn(workspace, sub);
}

interface Tree {
  open: ReadonlySet<string>;
  drop?: string;
  toggle(key: string): void;
  show(n: NoteDto): void;
  dropInto(level: NoteLevel, folder: string): React.HTMLAttributes<HTMLElement>;
  noteMenu(n: NoteDto): CtxEntry[];
  folderMenu(level: NoteLevel, f: NoteFolder): CtxEntry[];
}

function Level(props: {
  level: NoteLevel;
  title: string;
  data?: NotesDto;
  tree: Tree;
  onNewNote(): void;
  onNewFolder(): void;
}) {
  const { level, title, data, tree } = props;
  const isOpen = tree.open.has(level);
  const menu = contextMenu(() => [
    { label: "New Note…", onSelect: props.onNewNote },
    { label: "New Folder…", onSelect: props.onNewFolder },
  ]);
  return (
    <section
      className={`notes-level${tree.drop === `${level}:` ? " drop-target" : ""}`}
      {...tree.dropInto(level, "")}
    >
      <div className="notes-level-head" onContextMenu={menu}>
        <button type="button" className="notes-level-toggle" onClick={() => tree.toggle(level)}>
          <span className="chevron">{isOpen ? "▾" : "▸"}</span>
          <span className="name">{title}</span>
        </button>
        <IconButton icon="plus" label="New Note" onClick={props.onNewNote} />
        <IconButton icon="folderPlus" label="New Folder" onClick={props.onNewFolder} />
      </div>
      {isOpen && data && (
        <Folder level={level} folder={levelTree(data, level)} depth={0} tree={tree} />
      )}
    </section>
  );
}

function Folder({
  level,
  folder,
  depth,
  tree,
}: {
  level: NoteLevel;
  folder: NoteFolder;
  depth: number;
  tree: Tree;
}) {
  const pad = { paddingLeft: indent(depth) };
  return (
    <ul style={depth > 0 ? guideAt(depth) : undefined}>
      {folder.folders.map((f) => {
        const key = `${level}:${f.path}`;
        const isOpen = tree.open.has(key);
        return (
          <li key={f.path}>
            <button
              type="button"
              style={pad}
              className={tree.drop === key ? "drop-target" : undefined}
              onClick={() => tree.toggle(key)}
              onContextMenu={contextMenu(() => tree.folderMenu(level, f))}
              {...tree.dropInto(level, f.path)}
            >
              <span className="chevron">{isOpen ? "▾" : "▸"}</span>
              <Icon name="folder" size={13} />
              <span className="name">{f.name}</span>
            </button>
            {isOpen && <Folder level={level} folder={f} depth={depth + 1} tree={tree} />}
          </li>
        );
      })}
      {folder.notes.map((n) => (
        <li key={n.id}>
          <button
            type="button"
            style={pad}
            draggable
            onDragStart={(e) => {
              e.dataTransfer.setData(DRAG, n.id);
              e.dataTransfer.effectAllowed = "move";
            }}
            onClick={() => tree.show(n)}
            onContextMenu={contextMenu(() => tree.noteMenu(n))}
          >
            <span className="chevron" />
            <Icon name="note" size={13} />
            <span className="name">{n.name}</span>
            {n.updatedAt != null && <span className="notes-when">{ago(n.updatedAt)}</span>}
          </button>
        </li>
      ))}
    </ul>
  );
}
