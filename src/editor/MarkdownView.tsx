import { useEffect, useState } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import gfm from "remark-gfm";
import { HIGHLIGHT_NAMES } from "../bindings";
import { commands } from "../ipc";

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

/** Links show as text, since following one would navigate the app's window away. */
const components: Components = {
  a: ({ children }) => <span className="md-link">{children}</span>,
  code: ({ className, children }) => {
    const lang = /language-(\S+)/.exec(className ?? "")?.[1];
    const text = String(children ?? "").replace(/\n$/, "");
    return <code>{lang ? <Code lang={lang} text={text} /> : children}</code>;
  },
};

/** A markdown file rendered read-only, with GitHub's extensions (tables, task lists).
 * Raw HTML isn't rendered. */
export function MarkdownView({ text }: { text: string }) {
  return (
    <div className="markdown-view">
      <ReactMarkdown remarkPlugins={plugins} components={components}>
        {text}
      </ReactMarkdown>
    </div>
  );
}
