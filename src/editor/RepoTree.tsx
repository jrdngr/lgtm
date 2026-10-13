import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { FileEntry } from "../bindings";
import { commands, unwrap } from "../ipc";
import { useCurrentReview } from "../review/current";
import { guideAt, indent, toggled } from "../review/tree";
import { SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { useRepoChanged } from "../workspaces/useRepoChanged";
import { type Decor, decorate, decorLetter } from "./decor";
import { openFile } from "./open";
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
  useRepoChanged(() => {
    const keep = new Set(shown);
    setListings((m) => new Map([...m].filter(([d]) => keep.has(d))));
    load(shown);
  });
  return listings;
}

interface DirProps {
  dir: string;
  depth: number;
  listings: Map<string, Listing>;
  open: ReadonlySet<string>;
  onToggle(dir: string): void;
  /** How the current review changes files and folders, by path. */
  decor: ReturnType<typeof decorate>;
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

function Dir({ dir, depth, listings, open, onToggle, decor }: DirProps) {
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
              <button type="button" style={pad} className="dir" onClick={() => onToggle(e.path)}>
                <span className="chevron">{open.has(e.path) ? "▾" : "▸"}</span>
                <Name name={e.name} decor={decor.byDir.get(e.path)} dir />
              </button>
              {open.has(e.path) && (
                <Dir dir={e.path} depth={depth + 1} {...{ listings, open, onToggle, decor }} />
              )}
            </>
          ) : (
            <button
              type="button"
              style={pad}
              title={e.path}
              onClick={() => openFile(workspace, e.path)}
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
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
  const listings = useListings(workspace, open);
  const files = review?.files;
  const decor = useMemo(() => decorate(files?.map((f) => f.dto) ?? []), [files]);
  return (
    <div className="side-split">
      <SectionHeader title="Repo" />
      <div className="side-split-body file-tree">
        <Dir
          dir=""
          depth={0}
          listings={listings}
          open={open}
          onToggle={(d) => setOpen(toggled(open, d))}
          decor={decor}
        />
      </div>
    </div>
  );
}
