import { useLayoutEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import { components, gfmPlugins } from "../editor/markdownParts";
import { useHover } from "./hover";
import "./hover.css";

/** The language server's card about the symbol under the pointer: below it, or above when
 * there's no room. */
export function HoverCard() {
  const h = useHover();
  const ref = useRef<HTMLDivElement>(null);
  const [above, setAbove] = useState(false);
  useLayoutEffect(() => {
    const height = ref.current?.offsetHeight ?? 0;
    setAbove(!!h && h.top + height + 8 > window.innerHeight && h.bottom - height > 8);
  }, [h]);
  if (!h) return null;
  const left = Math.max(8, Math.min(h.x, window.innerWidth - 520));
  const style = above
    ? { left, bottom: window.innerHeight - h.bottom + 4 }
    : { left, top: h.top + 4 };
  return (
    <div ref={ref} className="hover-card" style={style}>
      <ReactMarkdown remarkPlugins={gfmPlugins} components={components}>
        {h.text}
      </ReactMarkdown>
    </div>
  );
}
