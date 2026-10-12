import { useMemo, useState } from "react";
import { useWorkspace } from "../workspaces/context";
import { type CurrentReview, useCurrentReview } from "./current";
import { STATUS_LABEL } from "./FileHeader";
import { isIdle } from "./idle";
import { matchesFilter, type Progress } from "./state";
import { buildTree, type TreeNode, toggled } from "./tree";
import "./FilesPanel.css";

export function Count({ p }: { p: Progress }) {
  const done = p.changed > 0 && p.unreviewed === 0;
  const pct = p.changed ? Math.round(((p.changed - p.unreviewed) / p.changed) * 100) : 100;
  return (
    <span className={`count${done ? " done" : ""}`} title={titleOf(p)}>
      <span className="bar">
        <span style={{ width: `${pct}%` }} />
      </span>
      {p.reviewed}/{p.changed}
    </span>
  );
}

function titleOf(p: Progress) {
  return `${p.reviewed} reviewed, ${p.skipped} skipped, ${p.flagged} flagged, ${p.unreviewed} left`;
}

interface TreeProps {
  depth: number;
  review: CurrentReview;
  closed: ReadonlySet<string>;
  onToggle(path: string): void;
}

function Children({ nodes, ...props }: TreeProps & { nodes: TreeNode[] }) {
  return (
    <ul>
      {nodes.map((c) => (
        <Node key={c.kind === "dir" ? c.path : c.file} node={c} {...props} />
      ))}
    </ul>
  );
}

function Node({ node, depth, review, closed, onToggle }: TreeProps & { node: TreeNode }) {
  const pad = { paddingLeft: 8 + depth * 12 };
  if (node.kind === "file") {
    const dto = review.files[node.file].dto;
    const check = review.checks[node.file];
    return (
      <li>
        <button
          type="button"
          style={pad}
          onClick={() => {
            console.debug("[lgtm] file click", { idle: isIdle(review), id: review.id });
            review.jump(node.file);
          }}
          title={dto.path}
        >
          <span className={`status status-${dto.status}`}>{STATUS_LABEL[dto.status]}</span>
          <span className={`name${check === "changedSinceReviewed" ? " amber" : ""}`}>
            {node.name}
          </span>
          {check === "checked" && <span className="checked">✓</span>}
          {node.progress.flagged > 0 && <span className="flag">⚑{node.progress.flagged}</span>}
          <Count p={node.progress} />
        </button>
      </li>
    );
  }
  const isClosed = closed.has(node.path);
  return (
    <li>
      <button type="button" style={pad} className="dir" onClick={() => onToggle(node.path)}>
        <span className="chevron">{isClosed ? "▸" : "▾"}</span>
        <span className="name">{node.name}</span>
        <Count p={node.progress} />
      </button>
      {!isClosed && (
        <Children
          nodes={node.children}
          depth={depth + 1}
          review={review}
          closed={closed}
          onToggle={onToggle}
        />
      )}
    </li>
  );
}

/** The current review's files as a tree with per-file and per-directory progress. It follows
 * the review pane's line filter. Click a file to jump to it. */
export function FilesPanel() {
  const review = useCurrentReview(useWorkspace());
  const [closed, setClosed] = useState<ReadonlySet<string>>(new Set());
  const tree = useMemo(
    () =>
      review &&
      buildTree(review.files, review.progress, (f) =>
        matchesFilter(review.progress[f], review.filter),
      ),
    [review],
  );
  if (!review || !tree) return <div className="placeholder">No review open</div>;
  if (tree.children.length === 0) {
    return (
      <div className="placeholder">
        {review.files.length === 0 ? "No changes" : `No ${review.filter} files`}
      </div>
    );
  }
  const toggle = (path: string) => setClosed(toggled(closed, path));
  return (
    <div className="file-tree">
      <div className="tree-total">
        <span>All files</span>
        <Count p={tree.progress} />
      </div>
      <Children nodes={tree.children} depth={0} review={review} closed={closed} onToggle={toggle} />
    </div>
  );
}
