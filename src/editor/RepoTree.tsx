import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { FileEntry } from "../bindings";
import { onRevealInFiles } from "../core/reveal";
import { useSavedSet } from "../core/savedState";
import { useWorkspace } from "../core/workspace";
import { commands, unwrap } from "../ipc";
import { useCurrentReview } from "../review/current";
import { guideAt, indent, toggled } from "../review/tree";
import { useSendFromMenu } from "../review/useSendText";
import { pathItems } from "../services/pathMenu";
import { type CtxEntry, contextMenu } from "../ui/contextMenu";
import { SectionHeader } from "../ui/SectionHeader";
import { useRepoChanged } from "../workspaces/useRepoChanged";
import { type Decor, decorate, decorLetter } from "./decor";
import { openFile } from "./open";
import { create, foldersOf, openToSide, rename, trash } from "./treeActions";
import { useTreeDrag } from "./useTreeDrag";
import "../review/FilesPanel.css";
import "./tree.css";

type Listing = FileEntry[] | { error: string };

/** Lists directories as they open, and lists them again when the repo changes. */
function useListings(workspace: string | null, open: ReadonlySet<string>) {
  const [listings, setListings] = useState(new Map<string, Listing>());
  /** Directories being listed, so renders before they land don't ask again. */
  const pending = useRef(new Set<string>());

  const load = useCallback(
    async (dirs: string[]) => {
      if (dirs.length === 0) return;
      for (const d of dirs) pending.current.add(d);
      const got = await Promise.all(
        dirs.map(async (dir): Promise<[string, Listing]> => {
          const r = await unwrap(commands.listFiles(workspace, dir)).catch((e: Error) => ({
            error: e.message,
          }));
          return [dir, r];
        }),
      );
      for (const d of dirs) pending.current.delete(d);
      setListings((m) => new Map([...m, ...got]));
    },
    [workspace],
  );

  const shown = useMemo(() => ["", ...open], [open]);
  useEffect(() => {
    load(shown.filter((d) => !listings.has(d) && !pending.current.has(d)));
  }, [shown, listings, load]);

  // Collapsed folders' listings go too, so reopening one lists it again.
  const reload = () => {
    const keep = new Set(shown);
    setListings((m) => new Map([...m].filter(([d]) => keep.has(d))));
    load(shown);
  };
  useRepoChanged(reload);
  return { listings, reload };
}

interface DirProps {
  dir: string;
  depth: number;
  listings: Map<string, Listing>;
  open: ReadonlySet<string>;
  onToggle(dir: string): void;
  /** How the current review changes files and folders, by path. */
  decor: ReturnType<typeof decorate>;
  menu(entry: FileEntry): CtxEntry[];
  /** The folder files dragged over the tree would land in. */
  dropDir?: string;
}

/** A name colored by how the review changes it, with the change's letter or, for a folder,
 * a dot, as VS Code shows them. */
function Name({ name, decor, dir }: { name: string; decor?: Decor; dir?: boolean }) {
  const cls = decor ? ` decor-${decor}` : "";
  return (
    <>
      <span className={`name${cls}`}>{name}</span>
      {decor && <span className={`decor${cls}`}>{dir ? "•" : decorLetter(decor)}</span>}
    </>
  );
}

function Dir({ dir, depth, listings, open, onToggle, decor, menu, dropDir }: DirProps) {
  const listing = listings.get(dir);
  const pad = { paddingLeft: indent(depth) };
  const workspace = useWorkspace();
  if (!listing)
    return (
      <div className="tree-note" style={pad}>
        …
      </div>
    );
  if ("error" in listing)
    return (
      <div className="tree-note error" style={pad}>
        {listing.error}
      </div>
    );
  return (
    <ul style={depth > 0 ? guideAt(depth) : undefined}>
      {listing.map((e) => (
        <li key={e.path}>
          {e.dir ? (
            <>
              <button
                type="button"
                style={pad}
                className={`dir${dropDir === e.path ? " drop-target" : ""}`}
                data-path={e.path}
                data-dir
                onClick={() => onToggle(e.path)}
                onContextMenu={contextMenu(() => menu(e))}
              >
                <span className="chevron">{open.has(e.path) ? "▾" : "▸"}</span>
                <Name name={e.name} decor={decor.byDir.get(e.path)} dir />
              </button>
              {open.has(e.path) && (
                <Dir
                  dir={e.path}
                  depth={depth + 1}
                  {...{ listings, open, onToggle, decor, menu, dropDir }}
                />
              )}
            </>
          ) : (
            <button
              type="button"
              style={pad}
              title={e.path}
              data-path={e.path}
              onClick={() => openFile(workspace, e.path)}
              onContextMenu={contextMenu(() => menu(e))}
            >
              <span className="chevron" />
              <Name name={e.name} decor={decor.byFile.get(e.path)} />
            </button>
          )}
        </li>
      ))}
    </ul>
  );
}

/** The workspace's working copy as a tree, without ignored files, colored by how the current
 * review changes it. Click a file to edit it.
 * Keyed by workspace, so a switch starts fresh. */
export function RepoTree() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const [open, setOpen] = useSavedSet(`files-open:${workspace ?? ""}`);
  const { listings, reload } = useListings(workspace, open);
  const files = review?.files;
  const decor = useMemo(() => decorate(files?.map((f) => f.dto) ?? []), [files]);
  const { sender } = useSendFromMenu(workspace);

  const body = useRef<HTMLDivElement>(null);
  // Opens the folders holding a revealed file, then focuses its row once it's listed.
  useEffect(
    () =>
      onRevealInFiles(workspace, (path) => {
        setOpen((o) => new Set([...o, ...foldersOf(path)]));
        let tries = 60;
        const find = () => {
          const row = body.current?.querySelector<HTMLElement>(`[data-path="${CSS.escape(path)}"]`);
          if (row) {
            row.scrollIntoView({ block: "nearest" });
            row.focus();
          } else if (--tries > 0) requestAnimationFrame(find);
        };
        requestAnimationFrame(find);
      }),
    [workspace, setOpen],
  );

  /** Lists the tree again with what holds `path` open, so a new or moved entry shows. */
  const madeAt = (path: string) => {
    setOpen((o) => new Set([...o, ...foldersOf(path)]));
    reload();
  };
  const { dropDir, dragProps } = useTreeDrag(workspace, madeAt);
  const menu = (e: FileEntry): CtxEntry[] => {
    const p = e.path;
    const edits: CtxEntry[] = [
      {
        label: "New File…",
        onSelect: () => create(workspace, e.dir ? p : (foldersOf(p).at(-1) ?? ""), false, madeAt),
      },
      e.dir && { label: "New Folder…", onSelect: () => create(workspace, p, true, madeAt) },
      { label: "Rename…", onSelect: () => rename(workspace, p, madeAt) },
      { label: "Move to Trash…", onSelect: () => trash(workspace, p, reload) },
    ];
    if (e.dir) {
      return [
        {
          label: open.has(p) ? "Collapse" : "Expand",
          onSelect: () => setOpen(toggled(open, p)),
        },
        { label: "Collapse All", onSelect: () => setOpen(new Set()), disabled: open.size === 0 },
        "-",
        ...edits,
        "-",
        ...pathItems(workspace, p),
      ];
    }
    return [
      { label: "Open", onSelect: () => openFile(workspace, p) },
      { label: "Open to the Side", onSelect: () => openToSide(workspace, p) },
      "-",
      ...edits,
      "-",
      ...pathItems(workspace, p, { send: sender }),
    ];
  };
  return (
    <div className="side-split">
      <SectionHeader title="Files" />
      <div
        className={`side-split-body file-tree${dropDir === "" ? " drop-target" : ""}`}
        ref={body}
        {...dragProps}
      >
        <Dir
          dir=""
          depth={0}
          listings={listings}
          open={open}
          onToggle={(d) => setOpen(toggled(open, d))}
          decor={decor}
          menu={menu}
          dropDir={dropDir}
        />
      </div>
    </div>
  );
}
