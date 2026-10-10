import { useMemo, useState } from "react";
import { useWorkspace } from "../workspaces/context";
import { type CurrentReview, useCurrentReview } from "./current";
import { STATUS_LABEL } from "./FileHeader";
import { matchesFilter, type Progress } from "./state";
import { buildTree, type TreeNode } from "./tree";

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

interface NodeProps {
  node: TreeNode;
  depth: number;
  review: CurrentReview;
  closed: ReadonlySet<string>;
  onToggle(path: string): void;
}

function Node({ node, depth, review, closed, onToggle }: NodeProps) {
  const pad = { paddingLeft: 8 + depth * 12 };
  if (node.kind === "file") {
    const dto = review.files[node.file].dto;
    const check = review.checks[node.file];
    return (
      <li>
        <button type="button" style={pad} onClick={() => review.jump(node.file)} title={dto.path}>
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
        <ul>
          {node.children.map((c) => (
            <Node
              key={c.kind === "dir" ? c.path : c.file}
              node={c}
              depth={depth + 1}
              review={review}
              closed={closed}
              onToggle={onToggle}
            />
          ))}
        </ul>
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
  const toggle = (path: string) => {
    const next = new Set(closed);
    if (!next.delete(path)) next.add(path);
    setClosed(next);
  };
  return (
    <div className="file-tree">
      <div className="tree-total">
        <span>All files</span>
        <Count p={tree.progress} />
      </div>
      <ul>
        {tree.children.map((c) => (
          <Node
            key={c.kind === "dir" ? c.path : c.file}
            node={c}
            depth={0}
            review={review}
            closed={closed}
            onToggle={toggle}
          />
        ))}
      </ul>
    </div>
  );
}
