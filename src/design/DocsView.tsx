import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CommentKind } from "../bindings";
import { MarkdownView } from "../editor/MarkdownView";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import { isMac } from "../workbench/keys";
import type { WorkspaceName } from "../workspaces/context";
import { CommentMargin } from "./CommentMargin";
import { changedLines, type Design, fileName } from "./model";
import { act, type Busy, runStep, setError, setView, showDesign } from "./store";

/** A selection in the rendered doc, by source lines. */
interface Picked {
  quote: string;
  start: number;
  end: number;
  top: number;
  left: number;
  kind?: CommentKind;
}

interface Props {
  ws: WorkspaceName;
  d: Design;
  doc: string;
  busy?: Busy;
  streams: Record<string, string>;
}

const lineOf = (n: Node | null, attr: "start" | "end") => {
  const el = (n instanceof Element ? n : n?.parentElement)?.closest<HTMLElement>("[data-start]");
  return el ? Number(el.dataset[attr]) : undefined;
};

/** One of the design's markdown files, rendered. Select a passage to comment on it (queued
 * until applied) or ask about it. Blocks changed since you last looked are marked. */
export function DocsView({ ws, d, doc, busy, streams }: Props) {
  const [file, setFile] = useState<{ text: string; seen: string | null }>();
  const [picked, setPicked] = useState<Picked>();
  const [body, setBody] = useState("");
  const bodyRef = useRef<HTMLDivElement>(null);
  const slug = d.slug;

  // Reloads whenever the design does, which is whenever its files change.
  useEffect(() => {
    let live = true;
    void d;
    commands.designFile(ws, slug, doc).then((r) => {
      if (!live) return;
      if (r.status === "error") return setError(ws, r.error);
      setFile(r.data);
      // A first look sees all of it.
      if (r.data.seen === null) commands.markDesignSeen(ws, slug, doc, r.data.text);
    });
    return () => {
      live = false;
    };
  }, [ws, slug, doc, d]);

  const marks = useMemo(
    () =>
      file?.seen != null && file.seen !== file.text
        ? changedLines(file.seen, file.text)
        : undefined,
    [file],
  );
  const comments = d.comments.filter((c) => c.path === doc);
  const queued = d.comments.filter((c) => c.kind === "change" && c.state === "queued");

  async function markSeen() {
    if (!file) return;
    await commands.markDesignSeen(ws, slug, doc, file.text);
    setFile({ ...file, seen: file.text });
  }

  function jump(dir: 1 | -1) {
    const blocks = [...(bodyRef.current?.querySelectorAll<HTMLElement>(".md-changed") ?? [])];
    const mid = bodyRef.current?.getBoundingClientRect().top ?? 0;
    const at = blocks.findIndex((b) => b.getBoundingClientRect().top > mid + 4);
    const i = dir === 1 ? at : (at === -1 ? blocks.length : at) - 1;
    blocks[i === -1 && dir === 1 ? 0 : Math.max(i, 0)]?.scrollIntoView({ block: "start" });
  }

  const pick = useCallback(() => {
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || !bodyRef.current?.contains(sel.anchorNode))
      return setPicked((p) => (p?.kind ? p : undefined));
    const a = lineOf(sel.anchorNode, "start");
    const b = lineOf(sel.focusNode, "start");
    const ea = lineOf(sel.anchorNode, "end");
    const eb = lineOf(sel.focusNode, "end");
    if (a === undefined || b === undefined) return;
    const rect = sel.getRangeAt(0).getBoundingClientRect();
    setPicked({
      quote: sel.toString().trim(),
      start: Math.min(a, b),
      end: Math.max(ea ?? a, eb ?? b),
      top: rect.bottom + 6,
      left: rect.left,
    });
    setBody("");
  }, []);

  async function send() {
    if (!picked?.kind || !body.trim()) return;
    const { quote, start, end, kind } = picked;
    const r = await act(
      ws,
      commands.addDesignComment(ws, slug, {
        path: doc,
        quote,
        start,
        end,
        body: body.trim(),
        kind,
      }),
    );
    setPicked(undefined);
    window.getSelection()?.removeAllRanges();
    if (r.status === "error" || kind !== "ask") return;
    const id = Math.max(...r.data.comments.map((c) => c.id));
    runStep(
      ws,
      slug,
      {
        kind: "ask",
        about: `c${id}`,
        subject: `${d.name}: ${fileName(doc)}`,
        streams: true,
        open: () => showDesign(ws, slug, "docs", doc),
      },
      (ch) => commands.answerDesignComment(ws, slug, id, ch),
    );
  }

  function apply() {
    if (busy || !queued.length) return;
    runStep(
      ws,
      slug,
      {
        kind: "comments",
        subject: `${d.name}: ${queued.length} comments`,
        open: () => showDesign(ws, slug, "docs"),
      },
      (ch) => commands.applyDesignComments(ws, slug, ch),
    );
  }

  function onKey(e: React.KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (mod && e.shiftKey && e.key === "Enter") apply();
    else if (e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement)
      return;
    else if (mod || e.altKey) return;
    else if (picked && !picked.kind && e.key === "c") setPicked({ ...picked, kind: "change" });
    else if (picked && !picked.kind && e.key === "a") setPicked({ ...picked, kind: "ask" });
    else if (e.key === "Escape" && picked) setPicked(undefined);
    else if (e.key === "]" && marks) jump(1);
    else if (e.key === "[" && marks) jump(-1);
    else return;
    e.preventDefault();
  }

  const changes = marks ? marks.added.size + marks.removed.size : 0;

  return (
    <div className="design-docs" tabIndex={-1} onKeyDown={onKey}>
      <nav className="design-doclist">
        {d.docs.map((p) => (
          <button
            key={p}
            type="button"
            className={p === doc ? "current" : undefined}
            onClick={() => setView(ws, "docs", p)}
            title={p}
          >
            {fileName(p)}
            {d.comments.some((c) => c.path === p && c.state === "queued") && (
              <span className="pill amber">•</span>
            )}
          </button>
        ))}
      </nav>
      <div className="design-doc">
        <div className="design-doc-bar">
          <b>{doc}</b>
          {changes > 0 && (
            <>
              <span className="pill amber">changed since you looked</span>
              <button type="button" className="quiet" onClick={() => jump(-1)}>
                <kbd>[</kbd>
              </button>
              <button type="button" className="quiet" onClick={() => jump(1)}>
                <kbd>]</kbd>
              </button>
              <button type="button" className="quiet" onClick={markSeen}>
                Mark seen
              </button>
            </>
          )}
          <span className="spacer" />
          <button type="button" className="quiet" onClick={() => openFile(ws, `${d.dir}/${doc}`)}>
            Edit
          </button>
          {queued.length > 0 && (
            <button type="button" className="primary" disabled={!!busy} onClick={apply}>
              Apply {queued.length} comment{queued.length > 1 ? "s" : ""} <kbd>⇧⌘↵</kbd>
            </button>
          )}
        </div>
        <div ref={bodyRef} className="design-doc-body" onMouseUp={pick}>
          {file ? (
            file.text.trim() ? (
              <MarkdownView text={file.text} lines marks={marks} />
            ) : (
              <div className="placeholder">Empty</div>
            )
          ) : null}
        </div>
        {picked && (
          <div className="design-pop" style={{ top: picked.top, left: picked.left }}>
            {!picked.kind ? (
              <>
                <button type="button" onClick={() => setPicked({ ...picked, kind: "change" })}>
                  Comment <kbd>c</kbd>
                </button>
                <button type="button" onClick={() => setPicked({ ...picked, kind: "ask" })}>
                  Ask <kbd>a</kbd>
                </button>
              </>
            ) : (
              <textarea
                // biome-ignore lint/a11y/noAutofocus: the comment is what was asked for
                autoFocus
                rows={3}
                className="qq-input"
                value={body}
                placeholder={
                  picked.kind === "change"
                    ? "What to change here. ↵ queues it"
                    : "Your question. ↵ asks the agent"
                }
                onChange={(e) => setBody(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape") setPicked(undefined);
                  else if (e.key === "Enter" && !e.shiftKey) send();
                  else return;
                  e.preventDefault();
                  e.stopPropagation();
                }}
              />
            )}
          </div>
        )}
      </div>
      <CommentMargin
        ws={ws}
        slug={slug}
        comments={comments}
        streams={streams}
        busy={busy}
        onJump={(line) =>
          bodyRef.current
            ?.querySelector(`[data-start="${line}"]`)
            ?.scrollIntoView({ block: "center" })
        }
      />
    </div>
  );
}
