import { useMemo } from "react";
import { useSavedSet } from "../core/savedState";
import { useWorkspace } from "../core/workspace";
import { type CtxEntry, contextMenu } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { Ring } from "../ui/Ring";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { dirMenu, fileMenu } from "./changesMenu";
import { type CurrentReview, useCurrentReview } from "./current";
import { Diffstat } from "./Diffstat";
import { STATUS_LABEL } from "./FileHeader";
import { type LineStat, lineStat, sumStats } from "./lineStat";
import { MarkCounts } from "./MarkCounts";
import { type MarkSetup, useMarkSetup } from "./marks";
import { filterLabel, matchesFilter, type Progress, progressTitle } from "./state";
import { buildTree, dirPaths, guideAt, indent, type TreeNode, toggled } from "./tree";
import { useSendFromMenu } from "./useSendText";
import "./FilesPanel.css";

const isDone = (p: Progress) => p.changed > 0 && p.done === p.changed;

interface TreeProps {
  depth: number;
  review: CurrentReview;
  /** Lines added and removed, by file. */
  stats: LineStat[];
  setup: MarkSetup;
  closed: ReadonlySet<string>;
  onToggle(path: string): void;
  menu(node: TreeNode): CtxEntry[];
}

function Children({ nodes, ...props }: TreeProps & { nodes: TreeNode[] }) {
  return (
    <ul style={props.depth > 0 ? guideAt(props.depth) : undefined}>
      {nodes.map((c) => (
        <Node key={c.kind === "dir" ? c.path : c.file} node={c} {...props} />
      ))}
    </ul>
  );
}

function Node({
  node,
  depth,
  review,
  stats,
  setup,
  closed,
  onToggle,
  menu,
}: TreeProps & { node: TreeNode }) {
  const pad = { paddingLeft: indent(depth) };
  const onContextMenu = contextMenu(() => menu(node));
  const p = node.progress;
  if (node.kind === "file") {
    const dto = review.files[node.file].dto;
    const check = review.checks[node.file];
    const changed = check === "changedSinceReviewed";
    const cls = [
      (isDone(p) || check === "checked") && "done",
      dto.status === "removed" && "removed",
    ].filter(Boolean);
    return (
      <li>
        <button
          type="button"
          style={pad}
          className={cls.join(" ") || undefined}
          onClick={() => review.jump(node.file)}
          onContextMenu={onContextMenu}
          title={dto.path}
        >
          <span className="chevron" />
          <span className={`status status-${dto.status}`}>{STATUS_LABEL[dto.status]}</span>
          <span className={`name${changed ? " amber" : ""}`}>{node.name}</span>
          <Diffstat stat={stats[node.file]} />
          <MarkCounts progress={p} setup={setup} notDone />
          {p.changed > 0 && <Ring done={p.done / p.changed} title={progressTitle(p, setup)} />}
        </button>
      </li>
    );
  }
  const isClosed = closed.has(node.path);
  return (
    <li>
      <button
        type="button"
        style={pad}
        className="dir"
        onClick={() => onToggle(node.path)}
        onContextMenu={onContextMenu}
        title={progressTitle(p, setup)}
      >
        <span className="chevron">{isClosed ? "▸" : "▾"}</span>
        <span className="name">{node.name}</span>
        <span className="dir-count reveal">
          {p.done}/{p.changed}
        </span>
        {isDone(p) && <Ring done={1} />}
      </button>
      {!isClosed && (
        <Children
          nodes={node.children}
          depth={depth + 1}
          review={review}
          stats={stats}
          setup={setup}
          closed={closed}
          onToggle={onToggle}
          menu={menu}
        />
      )}
    </li>
  );
}

/** The current review's files as a tree with per-file progress, and per-folder counts on
 * hover. It follows the review pane's line filter. Click a file to jump to it. */
export function FilesPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const setup = useMarkSetup();
  const { sender } = useSendFromMenu(workspace);
  const [closed, setClosed] = useSavedSet(`changes-closed:${workspace ?? ""}`);
  const tree = useMemo(
    () =>
      review &&
      buildTree(review.files, review.progress, (f) =>
        matchesFilter(review.progress[f], review.filter),
      ),
    [review],
  );
  const stats = useMemo(() => review?.files.map((f) => lineStat(f.dto)) ?? [], [review?.files]);
  if (!review || !tree) {
    return (
      <div className="side-split">
        <SectionHeader title="Changes" />
        <Empty>No review open</Empty>
      </div>
    );
  }
  const p = tree.progress;
  const filter = filterLabel(review.filter, setup);
  const anyClosed = closed.size > 0;
  const total = sumStats(
    review.files.flatMap((_, f) =>
      matchesFilter(review.progress[f], review.filter) ? [stats[f]] : [],
    ),
  );
  return (
    <div className="side-split">
      <SectionHeader
        title="Changes"
        meta={
          <>
            <Diffstat stat={total} />
            {p.changed > 0 && review.filter !== "all" && <span>{filter}</span>}
            {p.changed > 0 && (
              <Ring
                done={p.done === p.changed ? 1 : Math.min(p.done / p.changed, 0.99)}
                title={progressTitle(p, setup)}
                percent
              />
            )}
          </>
        }
      >
        {tree.children.length > 0 && (
          <IconButton
            icon={anyClosed ? "expand" : "collapse"}
            label={anyClosed ? "Expand all folders" : "Collapse all folders"}
            onClick={() => setClosed(new Set(anyClosed ? [] : dirPaths(tree)))}
          />
        )}
      </SectionHeader>
      {tree.children.length === 0 ? (
        <Empty>{review.files.length === 0 ? "No changes" : `No ${filter} files`}</Empty>
      ) : (
        <div className="side-split-body file-tree">
          <Children
            nodes={tree.children}
            depth={0}
            review={review}
            stats={stats}
            setup={setup}
            closed={closed}
            onToggle={(path) => setClosed(toggled(closed, path))}
            menu={(n) =>
              n.kind === "file"
                ? fileMenu(workspace, review, setup, n.file, sender)
                : dirMenu(workspace, review, n, closed, setClosed)
            }
          />
        </div>
      )}
    </div>
  );
}
