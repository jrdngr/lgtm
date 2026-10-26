import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { Role } from "../terminal/registry";
import { Busy } from "../ui/Busy";
import { Icon } from "../ui/icons";
import { Glyph, since } from "../workspaces/AgentGlyph";
import type { AgentInfo } from "../workspaces/activity";
import "./close.css";

/** A running program as the dialog lists it: an agent the way the workspace's agent list shows
 * it, or else a ›_ and its name. */
export interface ProgramRow {
  key: string;
  name: string;
  agent?: AgentInfo;
  /** The pin of its terminal, which colors an agent's name. */
  role?: Role;
}

type FilesChoice = "open" | "saving" | "saved" | "discarded";

interface Props {
  /** Names of the files with unsaved edits. */
  files: string[];
  programs: ProgramRow[];
  /** Saves every file. Resolves to whether all saved. */
  save(): Promise<boolean>;
  onDone(close: boolean): void;
}

const FILE_TAGS: Partial<Record<FilesChoice, string>> = { saved: "Saved", discarded: "Not saved" };

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
          {both ? "Before closing" : files.length ? "Save changes?" : "Stop running programs?"}
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
              <div className="close-list">
                {files.map((name) => (
                  <div key={name} className="close-row">
                    <span className="close-name">{name}</span>
                  </div>
                ))}
              </div>
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
              <div className="close-list">
                {programs.map(({ key, name, agent, role }) => (
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
                ))}
              </div>
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
  files: string[],
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
