import { createElement, useEffect, useMemo, useState } from "react";
import ReactMarkdown, { type Components, type ExtraProps } from "react-markdown";
import gfm from "remark-gfm";
import { HIGHLIGHT_NAMES } from "../bindings";
import { commands } from "../ipc";
import { Mermaid } from "./Mermaid";

const plugins = [gfm];
const hlClass = HIGHLIGHT_NAMES.map((n) => `hl-${n.replace(".", "-")}`);

/** A fenced block's code, colored by the backend's tree-sitter highlighting for its language. */
function Code({ lang, text }: { lang: string; text: string }) {
  const [lines, setLines] = useState<[number | null, number][][]>();
  useEffect(() => {
    let live = true;
    commands.highlightText(`code.${lang}`, text).then((l) => live && setLines(l));
    return () => {
      live = false;
    };
  }, [lang, text]);
  if (!lines?.length) return <>{text}</>;
  const all = text.split("\n");
  return (
    <>
      {all.map((line, i) => {
        let at = 0;
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: lines of one block never reorder
          <span key={i}>
            {(lines[i] ?? []).map(([cls, len], j) => {
              const part = line.slice(at, at + len);
              at += len;
              return cls === null ? (
                part
              ) : (
                // biome-ignore lint/suspicious/noArrayIndexKey: segments of one line never reorder
                <span key={j} className={hlClass[cls]}>
                  {part}
                </span>
              );
            })}
            {line.slice(at)}
            {i < all.length - 1 && "\n"}
          </span>
        );
      })}
    </>
  );
}

/** Links show as text, since following one would navigate the app's window away. Mermaid
 * blocks draw as diagrams. */
const components: Components = {
  a: ({ children }) => <span className="md-link">{children}</span>,
  pre: ({ node, children, ...rest }) => {
    const code = node?.children[0];
    const cls = code?.type === "element" ? code.properties.className : undefined;
    if (Array.isArray(cls) && cls.includes("language-mermaid") && code?.type === "element") {
      const text = code.children.map((c) => (c.type === "text" ? c.value : "")).join("");
      return <Mermaid text={text.replace(/\n$/, "")} />;
    }
    return <pre {...rest}>{children}</pre>;
  },
  code: ({ className, children }) => {
    const lang = /language-(\S+)/.exec(className ?? "")?.[1];
    const text = String(children ?? "").replace(/\n$/, "");
    return <code>{lang ? <Code lang={lang} text={text} /> : children}</code>;
  },
};

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
  return out as Components;
}

/** A markdown file rendered read-only, with GitHub's extensions (tables, task lists).
 * Raw HTML isn't rendered. With `lines`, blocks carry their source lines and `marks`. */
export function MarkdownView({
  text,
  lines,
  marks,
}: {
  text: string;
  lines?: boolean;
  marks?: LineMarks;
}) {
  const parts = useMemo(() => (lines ? lineComponents(marks) : components), [lines, marks]);
  return (
    <div className="markdown-view">
      <ReactMarkdown remarkPlugins={plugins} components={parts}>
        {text}
      </ReactMarkdown>
    </div>
  );
}
