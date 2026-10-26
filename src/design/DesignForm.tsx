import { useEffect, useMemo, useRef, useState } from "react";
import type { DesignDto, DesignInput } from "../bindings";
import { isMac } from "../core/keys";
import { type WorkspaceName, workspaceRoot } from "../core/workspace";
import { commands, errorText, unwrap } from "../ipc";
import { addPaths, repoRelative } from "../qa/readFirstPaths";
import { PipeButton } from "../terminal/PipeButton";
import { coveredCount, pathChoices } from "./paths";
import "./design.css";

interface Props {
  workspace: WorkspaceName;
  /** The design to edit; a new one when absent. */
  design?: DesignDto;
  /** Saves it, and generates it when `generate`. */
  onDone(saved: DesignDto, generate: boolean): Promise<void>;
  onCancel(): void;
}

/** Creates or edits a design: its name, revision, files and folders, and prompt. Without files
 * it saves an empty design to draw on. */
export function DesignForm({ workspace, design, onDone, onCancel }: Props) {
  const [name, setName] = useState(design?.name ?? "");
  const [revision, setRevision] = useState(design?.revision ?? "@");
  const [bookmarks, setBookmarks] = useState<string[]>([]);
  const [files, setFiles] = useState<string[]>(design?.files ?? []);
  const [prompt, setPrompt] = useState(design?.prompt ?? "");
  const [all, setAll] = useState<string[]>([]);
  const [adding, setAdding] = useState(false);
  const [query, setQuery] = useState("");
  const [over, setOver] = useState(false);
  const [error, setError] = useState<string>();
  const first = useRef<HTMLInputElement>(null);

  useEffect(() => {
    first.current?.focus();
    unwrap(commands.designBookmarks(workspace)).then(setBookmarks, setError);
  }, [workspace]);

  // Typing a revision looks its files up once it settles.
  useEffect(() => {
    const rev = revision.trim();
    if (!rev) return setAll([]);
    let live = true;
    const t = setTimeout(() => {
      unwrap(commands.revisionFiles(workspace, rev)).then(
        (files) => live && setAll(files),
        () => live && setAll([]),
      );
    }, 300);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [workspace, revision]);

  const choices = useMemo(() => pathChoices(all, query, files), [all, query, files]);

  const save = async (generate: boolean) => {
    const input: DesignInput = { name: name.trim(), revision: revision.trim(), files, prompt };
    if (!input.name) return setError("Name it first");
    try {
      await onDone(
        await unwrap(commands.saveDesign(workspace, design?.id ?? null, input)),
        generate && files.length > 0,
      );
    } catch (e) {
      setError(errorText(e));
    }
  };

  const dropping = (e: React.DragEvent) => e.dataTransfer.types.includes("Files");
  const root = workspaceRoot(workspace);

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: Escape closes it from the form
    <div className="dsn-shade" onClick={onCancel}>
      <form
        className="dsn-form"
        onClick={(e) => e.stopPropagation()}
        onSubmit={(e) => {
          e.preventDefault();
          save(true);
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
          else if (e.key === "Enter" && (isMac ? e.metaKey : e.ctrlKey)) {
            e.preventDefault();
            save(true);
          }
        }}
      >
        <h2>{design ? "Edit Design" : "New Design"}</h2>
        <label className="dsn-field">
          <span>Name</span>
          <input ref={first} value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <label className="dsn-field">
          <span>Revision</span>
          <input
            list="dsn-revisions"
            value={revision}
            spellCheck={false}
            onChange={(e) => setRevision(e.target.value)}
          />
          <datalist id="dsn-revisions">
            {["@", ...bookmarks].map((b) => (
              <option key={b} value={b} />
            ))}
          </datalist>
        </label>
        <div className="dsn-field top">
          <span>Files</span>
          <div className="dsn-add">
            {!!files.length && (
              <div
                className={`dsn-files${over ? " over" : ""}`}
                onDragOver={(e) => {
                  if (!dropping(e)) return;
                  e.preventDefault();
                  setOver(true);
                }}
                onDragLeave={() => setOver(false)}
                onDrop={(e) => {
                  if (!dropping(e)) return;
                  e.preventDefault();
                  setOver(false);
                  commands.droppedPaths().then((ps) =>
                    setFiles((f) =>
                      addPaths(
                        f,
                        ps.map((p) => repoRelative(p, root)),
                      ),
                    ),
                  );
                }}
              >
                {files.map((f) => (
                  <div key={f} className="dsn-file">
                    <span className="p">{f}</span>
                    {f.endsWith("/") && <span className="n">{coveredCount(all, f)} files</span>}
                    <button
                      type="button"
                      aria-label={`Remove ${f}`}
                      onClick={() => setFiles(files.filter((x) => x !== f))}
                    >
                      ×
                    </button>
                  </div>
                ))}
              </div>
            )}
            {adding ? (
              <>
                <input
                  // biome-ignore lint/a11y/noAutofocus: Add… opens it to type in
                  autoFocus
                  value={query}
                  aria-label="Add files"
                  onChange={(e) => setQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Escape") {
                      e.stopPropagation();
                      setAdding(false);
                    } else if (e.key === "Enter" && !e.metaKey && !e.ctrlKey && choices[0]) {
                      e.preventDefault();
                      setFiles(addPaths(files, [choices[0]]));
                      setQuery("");
                    }
                  }}
                />
                {!!choices.length && (
                  <div className="dsn-sugg">
                    {choices.map((c) => (
                      <button
                        key={c}
                        type="button"
                        onClick={() => {
                          setFiles(addPaths(files, [c]));
                          setQuery("");
                        }}
                      >
                        {c}
                      </button>
                    ))}
                  </div>
                )}
              </>
            ) : (
              <button
                type="button"
                className="dsn-btn"
                onDragOver={(e) => {
                  if (dropping(e)) e.preventDefault();
                }}
                onDrop={(e) => {
                  if (!dropping(e)) return;
                  e.preventDefault();
                  commands.droppedPaths().then((ps) =>
                    setFiles((f) =>
                      addPaths(
                        f,
                        ps.map((p) => repoRelative(p, root)),
                      ),
                    ),
                  );
                }}
                onClick={() => setAdding(true)}
              >
                Add…
              </button>
            )}
          </div>
        </div>
        <label className="dsn-field top">
          <span>Prompt</span>
          <textarea rows={4} value={prompt} onChange={(e) => setPrompt(e.target.value)} />
        </label>
        {error && <div className="dsn-error">{error}</div>}
        <div className="dsn-actions">
          <button type="button" className="dsn-btn" onClick={onCancel}>
            Cancel
          </button>
          {design && files.length > 0 && (
            <button type="button" className="dsn-btn" onClick={() => save(false)}>
              Save
            </button>
          )}
          {files.length > 0 ? (
            <PipeButton background type="submit" className="sm" keys={isMac ? "⌘↵" : "Ctrl↵"}>
              Generate
            </PipeButton>
          ) : (
            <button type="submit" className="dsn-btn">
              {design ? "Save" : "Create"}
              <kbd>{isMac ? "⌘↵" : "Ctrl↵"}</kbd>
            </button>
          )}
        </div>
      </form>
    </div>
  );
}
