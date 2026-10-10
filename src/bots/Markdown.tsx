import ReactMarkdown, { type Components } from "react-markdown";
import breaks from "remark-breaks";

const plugins = [breaks];

/** Links show as text: following one would navigate the app's window away, and an agent could
 * put what it read into the URL. */
const components: Components = { a: ({ children }) => <span>{children}</span> };

/** Agent-written markdown. Line breaks are kept as written. Raw HTML isn't rendered and links
 * aren't followable, so an agent can't inject anything into the window. */
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
