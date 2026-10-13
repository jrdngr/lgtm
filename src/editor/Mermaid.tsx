import { useEffect, useId, useRef, useState } from "react";

let ready: Promise<typeof import("mermaid").default> | undefined;

/** Mermaid, loaded on first use and set up once for the dark theme. */
function mermaid() {
  ready ??= import("mermaid").then(({ default: m }) => {
    m.initialize({ startOnLoad: false, theme: "dark", securityLevel: "strict" });
    return m;
  });
  return ready;
}

/** A Mermaid diagram, or its source with the error when it doesn't parse. */
export function Mermaid({ text }: { text: string }) {
  const id = `mermaid-${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const ref = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string>();
  useEffect(() => {
    let live = true;
    mermaid()
      .then((m) => m.render(id, text))
      .then(({ svg }) => {
        if (!live || !ref.current) return;
        ref.current.innerHTML = svg;
        setError(undefined);
      })
      .catch((e) => live && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
    };
  }, [id, text]);
  if (error)
    return (
      <div className="mermaid-error">
        <div className="review-error">Diagram doesn't parse: {error}</div>
        <pre>{text}</pre>
      </div>
    );
  return <div ref={ref} className="mermaid-diagram" />;
}
