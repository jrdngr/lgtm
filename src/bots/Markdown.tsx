import ReactMarkdown from "react-markdown";
import breaks from "remark-breaks";
import { components, gfmPlugins } from "../editor/markdownParts";

const plugins = [...gfmPlugins, breaks];

/** Agent-written markdown with GitHub's extensions, Mermaid diagrams and highlighted code.
 * Line breaks are kept as written. Raw HTML isn't rendered and links aren't followable, so an
 * agent can't inject anything into the window or put what it read into a URL. */
export function Markdown({ children }: { children: string }) {
  return (
    <ReactMarkdown remarkPlugins={plugins} components={components}>
      {children}
    </ReactMarkdown>
  );
}

/** One line of agent text with its `code spans`, for places a block of markdown can't go. */
export function Inline({ children }: { children: string }) {
  return (
    <>
      {children.split("`").map((part, i) =>
        // biome-ignore lint/suspicious/noArrayIndexKey: the parts of one string never reorder
        i % 2 ? <code key={i}>{part}</code> : part,
      )}
    </>
  );
}
