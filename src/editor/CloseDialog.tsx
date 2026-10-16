import { useEffect, useRef } from "react";
import { createRoot } from "react-dom/client";
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
}

export type CloseChoice = "save" | "discard" | "stop" | "cancel";

interface Props {
  /** Names of the files with unsaved edits. */
  files: string[];
  programs: ProgramRow[];
  onDone(choice: CloseChoice): void;
}

function CloseDialog({ files, programs, onDone }: Props) {
  const mainRef = useRef<HTMLButtonElement>(null);
  useEffect(() => mainRef.current?.focus(), []);
  const both = files.length > 0 && programs.length > 0;
  return (
    <div
      className="close-scrim"
      onKeyDown={(e) => {
        if (e.key !== "Escape") return;
        e.preventDefault();
        onDone("cancel");
      }}
      onMouseDown={(e) => e.target === e.currentTarget && onDone("cancel")}
    >
      <div className="close-dialog" role="dialog" aria-labelledby="close-title">
        <h3 id="close-title">{files.length ? "Save changes?" : "Stop running programs?"}</h3>
        <div className="close-body">
          {files.length > 0 && (
            <section>
              {both && <h4>Unsaved</h4>}
              <div className="close-list">
                {files.map((name) => (
                  <div key={name} className="close-row">
                    <span className="close-name">{name}</span>
                  </div>
                ))}
              </div>
            </section>
          )}
          {programs.length > 0 && (
            <section>
              {both && <h4>Running</h4>}
              <div className="close-list">
                {programs.map(({ key, name, agent }) => (
                  <div key={key} className="close-row">
                    {agent ? (
                      <Glyph state={agent.state} />
                    ) : (
                      <span className="close-program">
                        <Icon name="terminal" size={12} />
                      </span>
                    )}
                    <span
                      className={`close-name${agent ? (agent.headless ? " headless" : " in-terminal") : ""}`}
                    >
                      {name}
                    </span>
                    {agent && <span className="close-ago">{since(agent.since)}</span>}
                  </div>
                ))}
              </div>
            </section>
          )}
        </div>
        <div className="close-foot">
          <button type="button" className="quiet" onClick={() => onDone("cancel")}>
            Cancel <kbd>Esc</kbd>
          </button>
          {files.length > 0 ? (
            <>
              <button type="button" onClick={() => onDone("discard")}>
                Don't Save
              </button>
              <button
                ref={mainRef}
                type="button"
                className="primary"
                onClick={() => onDone("save")}
              >
                {files.length === 1 ? "Save" : "Save All"}
              </button>
            </>
          ) : (
            <button
              ref={mainRef}
              type="button"
              className="close-stop"
              onClick={() => onDone("stop")}
            >
              Stop
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

/** Shows the dialog over the window and resolves to the user's choice. */
export function askClose(files: string[], programs: ProgramRow[]): Promise<CloseChoice> {
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  return new Promise((resolve) => {
    root.render(
      <CloseDialog
        files={files}
        programs={programs}
        onDone={(choice) => {
          root.unmount();
          host.remove();
          resolve(choice);
        }}
      />,
    );
  });
}
