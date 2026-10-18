import { Fragment, useState } from "react";
import type { Hunk, PreviewFileDto, PreviewThreadDto, Row } from "../bindings";
import { SIGN } from "../model/sign";
import { GhThread } from "./GhThread";
import "../review/comments.css";
import "../review/FileHeader.css";
import "../review/review.css";

/** A review thread, read-only. `where` names its code when it isn't shown beside it. */
export function PreviewThread({ thread, where }: { thread: PreviewThreadDto; where?: string }) {
  const { link } = thread;
  return (
    <div className={`comment-block gh preview-thread${link.resolved ? " resolved" : ""}`}>
      <div className="card-bar">
        <span className="card-source">{where ?? "GitHub"}</span>
        {link.resolved && <span className="pill green">✓ Resolved</span>}
        {link.outdated && <span className="pill">Outdated</span>}
      </div>
      {where && thread.hunk && <pre className="preview-hunk">{thread.hunk}</pre>}
      <GhThread link={link} />
    </div>
  );
}

function RowLine({ row }: { row: Row }) {
  return (
    <div className="row-line unified preview-row">
      <div className={`cell kind-${row.kind}`}>
        <span className="strip" />
        <span className="ln">{row.oldLine ?? ""}</span>
        <span className="ln">{row.newLine ?? ""}</span>
        <span className="sign">{SIGN[row.kind]}</span>
        <span className="code">{row.text}</span>
      </div>
    </div>
  );
}

function HunkRows({ hunk, threads }: { hunk: Hunk; threads: PreviewFileDto["threads"] }) {
  return (
    <>
      <div className="hunk-header preview-row">
        {"  "}@@ -{hunk.oldStart},{hunk.oldLines} +{hunk.newStart},{hunk.newLines} @@ {hunk.section}
      </div>
      {hunk.rows.map((row, r) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: a hunk's rows never change
        <Fragment key={r}>
          <RowLine row={row} />
          {threads
            .filter((t) => t.row === r)
            .map((t) => (
              <PreviewThread key={t.thread.link.thread} thread={t.thread} />
            ))}
        </Fragment>
      ))}
    </>
  );
}

/** One file of a PR's diff with its review threads under their lines. Click the header to
 * fold it. */
export function PreviewFile({ file }: { file: PreviewFileDto }) {
  const [open, setOpen] = useState(true);
  const n = file.threads.length;
  return (
    <div className="preview-file">
      <button
        type="button"
        className="file-header preview-file-header"
        onClick={() => setOpen(!open)}
      >
        <span>{open ? "▾" : "▸"}</span>
        <span className="path">{file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}</span>
        {file.binary && <span className="tag">binary</span>}
        <span className="spacer" />
        {n > 0 && <span className="file-count">{n === 1 ? "1 thread" : `${n} threads`}</span>}
      </button>
      {open &&
        file.hunks.map((hunk, h) => (
          <HunkRows
            // biome-ignore lint/suspicious/noArrayIndexKey: a file's hunks never change
            key={h}
            hunk={hunk}
            threads={file.threads.filter((t) => t.hunk === h)}
          />
        ))}
    </div>
  );
}
