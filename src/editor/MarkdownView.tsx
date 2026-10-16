import { createElement, useMemo } from "react";
import ReactMarkdown, { type Components, type ExtraProps } from "react-markdown";
import { components, gfmPlugins, LinkContext } from "./markdownParts";

/** Blocks that carry their source lines. */
const BLOCKS = ["p", "h1", "h2", "h3", "h4", "h5", "h6", "li", "blockquote", "tr", "hr"] as const;

/** Source line marks for a rendered file. */
export interface LineMarks {
  /** Lines new since the user last looked, from 1. */
  added: Set<number>;
  /** Lines after which lines are gone. */
  removed: Set<number>;
}

/** Components that tag each block with its source lines (`data-start`, `data-end`), and mark
 * blocks holding lines in `marks`. */
function lineComponents(marks?: LineMarks): Components {
  const out: Record<string, unknown> = { ...components };
  for (const tag of BLOCKS) {
    out[tag] = ({ node, className, ...rest }: { className?: string } & ExtraProps) => {
      const start = node?.position?.start.line ?? 0;
      const end = node?.position?.end.line ?? start;
      const has = (set: Set<number> | undefined, from: number) => {
        for (let l = from; l <= end; l++) if (set?.has(l)) return true;
        return false;
      };
      const classes = [
        className,
        has(marks?.added, start) && "md-changed",
        has(marks?.removed, start === 1 ? 0 : start) && "md-removed",
      ].filter(Boolean);
      return createElement(tag, {
        ...rest,
        className: classes.join(" ") || undefined,
        "data-start": start,
        "data-end": end,
      });
    };
  }
  // Code blocks keep their own rendering, inside a box that carries the lines.
  const Pre = components.pre as (props: ExtraProps) => React.ReactNode;
  out.pre = (props: ExtraProps) => {
    const start = props.node?.position?.start.line ?? 0;
    const end = props.node?.position?.end.line ?? start;
    return createElement(
      "div",
      { className: "md-lines", "data-start": start, "data-end": end },
      createElement(Pre, props),
    );
  };
  return out as Components;
}

/** A markdown file rendered read-only, with GitHub's extensions (tables, task lists).
 * Raw HTML isn't rendered. With `lines`, blocks carry their source lines and `marks`. */
export function MarkdownView({
  text,
  lines,
  marks,
  onLink,
}: {
  text: string;
  lines?: boolean;
  marks?: LineMarks;
  /** Follows a ⌘-clicked link, in place of opening only web addresses. */
  onLink?: (href: string) => void;
}) {
  const parts = useMemo(() => (lines ? lineComponents(marks) : components), [lines, marks]);
  const view = (
    <div className="markdown-view">
      <ReactMarkdown remarkPlugins={gfmPlugins} components={parts}>
        {text}
      </ReactMarkdown>
    </div>
  );
  return onLink ? <LinkContext.Provider value={onLink}>{view}</LinkContext.Provider> : view;
}
