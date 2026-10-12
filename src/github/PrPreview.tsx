import type { IDockviewPanelProps } from "dockview-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { PrPreviewDto } from "../bindings";
import { commands } from "../ipc";
import { PreviewFile, PreviewThread } from "./PreviewDiff";
import { PrPosts } from "./PrPosts";
import "./github.css";

const STATES: Record<string, string> = { CLOSED: " · closed", MERGED: " · merged" };

/** A PR read straight from GitHub, without checking it out: its conversation and review
 * summaries, then its diff with review threads under their lines. Read-only. */
export function PrPreview({ params }: IDockviewPanelProps<{ url: string }>) {
  const { url } = params;
  // Keyed by the URL asked for: a reply for an earlier PR, still on its way, is dropped.
  const [got, setGot] = useState<{ url: string; pr?: PrPreviewDto; error?: string }>();
  const latest = useRef(url);
  latest.current = url;

  const load = useCallback(async () => {
    const r = await commands.prPreview(url);
    if (latest.current !== url) return;
    setGot(r.status === "ok" ? { url, pr: r.data } : { url, error: r.error });
  }, [url]);

  useEffect(() => {
    load();
  }, [load]);

  const current = got?.url === url ? got : undefined;
  const shown = current?.pr;
  const error = current?.error;

  return (
    <div className="pr-preview">
      {error && (
        <div className="review-error">
          {error}{" "}
          <button type="button" onClick={load}>
            Retry
          </button>
        </div>
      )}
      {!current && <div className="placeholder">Loading</div>}
      {shown && (
        <>
          <div className="pr-panel">
            <div className="pr-head">
              <div className="pr-title">
                #{shown.number} {shown.title}
              </div>
              <div className="pr-meta">
                {shown.author} · {shown.headRef} → {shown.baseRef}
                {STATES[shown.state] ?? ""}
              </div>
              <div className="pr-meta">{shown.url}</div>
              <div className="pr-actions">
                <button type="button" onClick={load}>
                  Reload
                </button>
              </div>
            </div>
            <PrPosts posts={shown.posts} />
            {shown.elsewhere.map((t) => (
              <PreviewThread key={t.link.thread} thread={t} where={t.path} />
            ))}
          </div>
          {shown.files.map((f) => (
            <PreviewFile key={f.path} file={f} />
          ))}
        </>
      )}
    </div>
  );
}
