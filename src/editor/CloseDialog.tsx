import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { Role } from "../terminal/registry";
import { Busy } from "../ui/Busy";
import { Icon } from "../ui/icons";
import { Glyph, since } from "../workspaces/AgentGlyph";
import type { AgentInfo } from "../workspaces/activity";
import { grouped, type Where } from "./closeGroups";
import "./close.css";

export type { Where };

/** A file with unsaved edits. */
export interface FileRow {
  name: string;
  where?: Where;
}

/** A running program as the dialog lists it: an agent the way the workspace's agent list shows
 * it, or else a ›_ and its name. */
export interface ProgramRow {
  key: string;
  name: string;
  agent?: AgentInfo;
  /** The pin of its terminal, which colors an agent's name. */
  role?: Role;
  where?: Where;
}

type FilesChoice = "open" | "saving" | "saved" | "discarded";

interface Props {
  files: FileRow[];
  programs: ProgramRow[];
  /** Saves every file. Resolves to whether all saved. */
  save(): Promise<boolean>;
  onDone(close: boolean): void;
}

const FILE_TAGS: Partial<Record<FilesChoice, string>> = { saved: "Saved", discarded: "Not saved" };

/** The list box: rows as they come, or under their repos and workspaces when they say where. */
function List<T extends { where?: Where }>({
  rows,
  row,
}: {
  rows: T[];
  row: (r: T, i: number) => React.ReactNode;
}) {
  if (!rows.some((r) => r.where)) return <div className="close-list">{rows.map(row)}</div>;
  let i = 0;
  return (
    <div className="close-list grouped">
      {grouped(rows).map((r) => (
        <div key={r.repo} className="close-repo">
          <div className="close-repo-name">{r.repo}</div>
          {r.workspaces.map((w) => (
            <div key={w.workspace}>
              <div className="close-ws-name">{w.workspace}</div>
              {w.rows.map((x) => row(x, i++))}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

/** Asks about unsaved files and running programs. With both, each section has its own buttons
 * and the dialog closes once both are decided. */
function CloseDialog({ files, programs, save, onDone }: Props) {
  const mainRef = useRef<HTMLButtonElement>(null);
  const [filesChoice, setFilesChoice] = useState<FilesChoice>("open");
  const [stop, setStop] = useState(false);
  const both = files.length > 0 && programs.length > 0;
  const filesDone = files.length === 0 || filesChoice === "saved" || filesChoice === "discarded";
  const programsDone = programs.length === 0 || stop;

  useEffect(() => mainRef.current?.focus(), []);
  useEffect(() => {
    if (filesDone && programsDone) onDone(true);
  }, [filesDone, programsDone, onDone]);

  async function saveAll() {
    setFilesChoice("saving");
    if (await save()) setFilesChoice("saved");
    else onDone(false);
  }

  const fileButtons = (
    <>
      <button
        type="button"
        disabled={filesChoice === "saving"}
        onClick={() => setFilesChoice("discarded")}
      >
        Don't Save
      </button>
      <button
        ref={mainRef}
        type="button"
        className="primary"
        disabled={filesChoice === "saving"}
        onClick={saveAll}
      >
        {filesChoice === "saving" ? <Busy>Saving…</Busy> : files.length === 1 ? "Save" : "Save All"}
      </button>
    </>
  );
  const stopButton = (
    <button
      ref={files.length ? undefined : mainRef}
      type="button"
      className="close-stop"
      onClick={() => setStop(true)}
    >
      Stop
    </button>
  );
  const fileTag = FILE_TAGS[filesChoice];

  return (
    <div
      className="close-scrim"
      onKeyDown={(e) => {
        if (e.key !== "Escape") return;
        e.preventDefault();
        onDone(false);
      }}
      onMouseDown={(e) => e.target === e.currentTarget && onDone(false)}
    >
      <div className="close-dialog" role="dialog" aria-labelledby="close-title">
        <h3 id="close-title">
          {both
            ? `Before ${[...files, ...programs].some((r) => r.where) ? "quitting" : "closing"}`
            : files.length
              ? "Save changes?"
              : "Stop running programs?"}
        </h3>
        <div className="close-body">
          {files.length > 0 && (
            <section className={fileTag ? "decided" : undefined}>
              {both && (
                <h4>
                  Unsaved
                  {fileTag && <span className="close-tag">{fileTag}</span>}
                </h4>
              )}
              <List
                rows={files}
                row={({ name }, i) => (
                  <div key={i} className="close-row">
                    <span className="close-name">{name}</span>
                  </div>
                )}
              />
              {both && !fileTag && <div className="close-section-foot">{fileButtons}</div>}
            </section>
          )}
          {programs.length > 0 && (
            <section className={stop ? "decided" : undefined}>
              {both && (
                <h4>
                  Running
                  {stop && <span className="close-tag">Stopping</span>}
                </h4>
              )}
              <List
                rows={programs}
                row={({ key, name, agent, role }) => (
                  <div key={key} className="close-row">
                    {agent ? (
                      <Glyph state={agent.state} />
                    ) : (
                      <span className="close-program">
                        <Icon name="terminal" size={12} />
                      </span>
                    )}
                    <span className={`close-name${agent ? " in-terminal" : ""}`} data-send={role}>
                      {name}
                    </span>
                    {agent && <span className="close-ago">{since(agent.since)}</span>}
                  </div>
                )}
              />
              {both && !stop && <div className="close-section-foot">{stopButton}</div>}
            </section>
          )}
        </div>
        <div className="close-foot">
          <button type="button" className="quiet" onClick={() => onDone(false)}>
            Cancel <kbd>Esc</kbd>
          </button>
          {!both && (files.length ? fileButtons : stopButton)}
        </div>
      </div>
    </div>
  );
}

/** Shows the dialog over the window. Resolves to whether everything may close: the files were
 * saved or let go and the programs may stop. A failed save resolves to false. */
export function askClose(
  files: FileRow[],
  programs: ProgramRow[],
  save: () => Promise<boolean>,
): Promise<boolean> {
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  return new Promise((resolve) => {
    let done = false;
    root.render(
      <CloseDialog
        files={files}
        programs={programs}
        save={save}
        onDone={(close) => {
          if (done) return;
          done = true;
          // After the render that called it.
          queueMicrotask(() => {
            root.unmount();
            host.remove();
          });
          resolve(close);
        }}
      />,
    );
  });
}
