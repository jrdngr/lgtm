import { useEffect, useState } from "react";
import type { ArtifactKind } from "../bindings";
import { Mermaid } from "../editor/Mermaid";
import { openFile } from "../editor/open";
import { commands } from "../ipc";
import { isMac } from "../workbench/keys";
import type { WorkspaceName } from "../workspaces/context";
import { type Design, fileName } from "./model";
import { type Busy, runStep, setError, setView, showDesign } from "./store";

const KINDS: [ArtifactKind, string, string][] = [
  ["doc", "Doc", "A markdown doc, like an overview or a how-to"],
  ["diagram", "Diagram", "A Mermaid diagram"],
  ["htmlPage", "HTML page", "A polished page that stands alone"],
];

/** Pages can't run scripts or load anything: what the agent wrote is shown, not trusted. */
const CSP =
  "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:\">";

/** The page with the policy first in its head, keeping its doctype first. */
const withCsp = (page: string) =>
  /<head[^>]*>/i.test(page) ? page.replace(/<head[^>]*>/i, (head) => head + CSP) : CSP + page;

interface Props {
  ws: WorkspaceName;
  d: Design;
  busy?: Busy;
}

/** What the design has made: docs, diagrams and HTML pages, with a preview of the picked one,
 * and asking the agent for more. */
export function FilesView({ ws, d, busy }: Props) {
  const [kind, setKind] = useState<ArtifactKind>("doc");
  const [about, setAbout] = useState("");
  const [picked, setPicked] = useState<string>();
  const [text, setText] = useState<string>();
  const slug = d.slug;

  useEffect(() => {
    let live = true;
    void d;
    setText(undefined);
    if (!picked) return;
    commands.designFile(ws, slug, picked).then((r) => {
      if (!live) return;
      if (r.status === "error") setError(ws, r.error);
      else setText(r.data.text);
    });
    return () => {
      live = false;
    };
  }, [ws, slug, picked, d]);

  const open = () => showDesign(ws, slug, "files");

  async function make() {
    if (busy || !about.trim()) return;
    const r = await runStep(
      ws,
      slug,
      { kind: "artifact", about: kind, subject: `${d.name}: ${about.trim().slice(0, 40)}`, open },
      (ch) => commands.makeDesignArtifact(ws, slug, kind, about.trim(), ch),
      (m) => m.design,
    );
    if (r.status === "error") return;
    setAbout("");
    show(r.data.path);
  }

  async function html(source: string) {
    if (busy) return;
    const r = await runStep(
      ws,
      slug,
      { kind: "html", about: source, subject: `${d.name}: HTML for ${fileName(source)}`, open },
      (ch) => commands.makeDesignHtml(ws, slug, source, ch),
      (m) => m.design,
    );
    if (r.status === "ok") setPicked(r.data.path);
  }

  function show(path: string) {
    if (path.endsWith(".md")) setView(ws, "docs", path);
    else setPicked(path);
  }

  const htmlFor = (source: string) => d.html.find((h) => h.source === source);
  const makeHtml = (source: string) => {
    const page = htmlFor(source);
    return (
      <button type="button" className="quiet" disabled={!!busy} onClick={() => html(source)}>
        {page ? (page.stale ? "Regenerate HTML" : "Remake HTML") : "Make HTML"}
      </button>
    );
  };

  return (
    <div className="design-files">
      <div className="design-file-list">
        <section>
          <div className="qq-label">Docs</div>
          {d.docs.map((p) => (
            <div key={p} className="design-file">
              <button type="button" className="design-file-name" onClick={() => show(p)}>
                {p}
              </button>
              {makeHtml(p)}
            </div>
          ))}
        </section>
        {d.diagrams.length > 0 && (
          <section>
            <div className="qq-label">Diagrams</div>
            {d.diagrams.map((p) => (
              <div key={p} className={`design-file${picked === p ? " current" : ""}`}>
                <button type="button" className="design-file-name" onClick={() => show(p)}>
                  {p}
                </button>
                {makeHtml(p)}
              </div>
            ))}
          </section>
        )}
        {d.html.length > 0 && (
          <section>
            <div className="qq-label">HTML pages</div>
            {d.html.map((h) => (
              <div key={h.path} className={`design-file${picked === h.path ? " current" : ""}`}>
                <button type="button" className="design-file-name" onClick={() => show(h.path)}>
                  {h.path}
                </button>
                {h.stale && <span className="pill amber">source changed</span>}
                <button
                  type="button"
                  className="quiet"
                  onClick={() =>
                    commands
                      .openDesignFile(ws, slug, h.path)
                      .then((r) => r.status === "error" && setError(ws, r.error))
                  }
                >
                  Open in browser
                </button>
              </div>
            ))}
          </section>
        )}
        <section className="design-make">
          <div className="qq-label">Make</div>
          <div className="design-kinds">
            {KINDS.map(([k, label, help]) => (
              <button
                key={k}
                type="button"
                className={k === kind ? "current" : undefined}
                title={help}
                onClick={() => setKind(k)}
              >
                {label}
              </button>
            ))}
          </div>
          <textarea
            className="qq-input"
            rows={3}
            value={about}
            placeholder="What it covers and who it's for. ⌘↵ makes it"
            onChange={(e) => setAbout(e.target.value)}
            onKeyDown={(e) => {
              if ((isMac ? e.metaKey : e.ctrlKey) && e.key === "Enter") {
                e.preventDefault();
                make();
              }
            }}
          />
          <button
            type="button"
            className="primary"
            disabled={!!busy || !about.trim()}
            onClick={make}
          >
            Make {KINDS.find(([k]) => k === kind)?.[1]} <kbd>⌘↵</kbd>
          </button>
        </section>
      </div>
      <div className="design-preview">
        {!picked ? (
          <div className="placeholder">Pick a diagram or page to preview it</div>
        ) : text === undefined ? null : picked.endsWith(".mmd") ? (
          <>
            <div className="design-doc-bar">
              <b>{picked}</b>
              <span className="spacer" />
              <button
                type="button"
                className="quiet"
                onClick={() => openFile(ws, `${d.dir}/${picked}`)}
              >
                Edit
              </button>
            </div>
            <div className="design-diagram">
              <Mermaid text={text} />
            </div>
          </>
        ) : (
          <iframe title={picked} className="design-frame" sandbox="" srcDoc={withCsp(text)} />
        )}
      </div>
    </div>
  );
}
