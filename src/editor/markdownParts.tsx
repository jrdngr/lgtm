import { useEffect, useState } from "react";
import type { Components } from "react-markdown";
import gfm from "remark-gfm";
import { HIGHLIGHT_NAMES } from "../bindings";
import { commands } from "../ipc";
import { Mermaid } from "./Mermaid";
import "../review/syntax.css";
import "./markdown.css";

/** GitHub's extensions: tables, task lists, strikethrough and footnotes. */
export const gfmPlugins = [gfm];
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
 * blocks draw as diagrams, fenced code is highlighted, and wide tables scroll. */
export const components: Components = {
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
  table: ({ node, ...rest }) => (
    <div className="md-table">
      <table {...rest} />
    </div>
  ),
  code: ({ className, children }) => {
    const lang = /language-(\S+)/.exec(className ?? "")?.[1];
    const text = String(children ?? "").replace(/\n$/, "");
    return <code>{lang ? <Code lang={lang} text={text} /> : children}</code>;
  },
};
