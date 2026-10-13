import { useMemo, useState } from "react";
import { IconButton } from "../ui/icons";
import { Ring } from "../ui/Ring";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { type CurrentReview, useCurrentReview } from "./current";
import { STATUS_LABEL } from "./FileHeader";
import { MarkCounts } from "./MarkCounts";
import { type MarkSetup, useMarkSetup } from "./marks";
import { filterLabel, matchesFilter, type Progress, plural, progressTitle } from "./state";
import { buildTree, dirPaths, guideAt, indent, type TreeNode, toggled } from "./tree";
import "./FilesPanel.css";

const isDone = (p: Progress) => p.changed > 0 && p.done === p.changed;

interface TreeProps {
  depth: number;
  review: CurrentReview;
  setup: MarkSetup;
  closed: ReadonlySet<string>;
  onToggle(path: string): void;
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

function Node({ node, depth, review, setup, closed, onToggle }: TreeProps & { node: TreeNode }) {
  const pad = { paddingLeft: indent(depth) };
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
          title={dto.path}
        >
          <span className="chevron" />
          <span className={`status status-${dto.status}`}>{STATUS_LABEL[dto.status]}</span>
          <span className={`name${changed ? " amber" : ""}`}>{node.name}</span>
          {changed && <span className="pill amber">changed</span>}
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
          setup={setup}
          closed={closed}
          onToggle={onToggle}
        />
      )}
    </li>
  );
}

/** The current review's files as a tree with per-file progress, and per-folder counts on
 * hover. It follows the review pane's line filter. Click a file to jump to it. */
export function FilesPanel() {
  const review = useCurrentReview(useWorkspace());
  const setup = useMarkSetup();
  const [closed, setClosed] = useState<ReadonlySet<string>>(new Set());
  const tree = useMemo(
    () =>
      review &&
      buildTree(review.files, review.progress, (f) =>
        matchesFilter(review.progress[f], review.filter),
      ),
    [review],
  );
  if (!review || !tree) {
    return (
      <div className="side-split">
        <SectionHeader title="Changes" />
        <Empty>No review open</Empty>
      </div>
    );
  }
  const p = tree.progress;
  const pct = p.changed ? Math.round((p.done / p.changed) * 100) : 100;
  const filter = filterLabel(review.filter, setup);
  const anyClosed = closed.size > 0;
  return (
    <div className="side-split">
      <SectionHeader
        title="Changes"
        meta={
          p.changed > 0 && (
            <span title={progressTitle(p, setup)}>
              {review.filter === "all" ? "" : `${filter} · `}
              {p.done} of {p.changed} {plural(p.changed)} done · {pct}%
            </span>
          )
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
            setup={setup}
            closed={closed}
            onToggle={(path) => setClosed(toggled(closed, path))}
          />
        </div>
      )}
    </div>
  );
}
