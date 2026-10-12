import { useCallback, useEffect, useRef, useState } from "react";
import type { NearbyPrDto, NearbyPrsDto } from "../bindings";
import { closePrPreview, openPrPreview } from "../github/preview";
import { type CommandResult, commands, errorText } from "../ipc";
import { useGithubOff } from "../tools/agents";
import { dockOf } from "../workbench/docks";
import type { WorkspaceName } from "./context";

interface Props {
  /** The workspace whose `@` the PRs are found from. */
  workspace: WorkspaceName;
  /** Called with the PR's workspace, and whether it's new and has a setup script. */
  onOpened(workspace: string, setup: boolean): void;
  onError(message: string): void;
}

/** A gh or remote problem as a quiet note: listing PRs is a convenience. */
function quiet(error: string): string {
  if (/auth login|not logged in/i.test(error)) return "Sign in with `gh auth login` to list PRs";
  if (/no GitHub remote/i.test(error)) return "No GitHub remote";
  return error;
}

/** Why the list is empty, if it is. */
function emptyNote({ prs, bookmarks }: NearbyPrsDto): string | undefined {
  if (prs.length) return undefined;
  if (!bookmarks.length) return "No bookmarks pushed to GitHub on this workspace's stack";
  const names = bookmarks.slice(0, 3).join(", ") + (bookmarks.length > 3 ? ", …" : "");
  return `No open PRs for ${names}`;
}

/** Open PRs of the bookmarks pushed from the workspace's stack, nearest `@` first. Click one
 * to preview it in the workspace's dock; double-click or Enter reviews it in its own
 * workspace. Loads when the workspace changes and on Refresh. */
export function NearbyPrs({ workspace, onOpened, onError }: Props) {
  const githubOff = useGithubOff();
  const [prs, setPrs] = useState<NearbyPrDto[]>();
  const [note, setNote] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [opening, setOpening] = useState<number>();
  const [selected, setSelected] = useState<number>();

  // The workspace the latest load is for: a reply for an earlier one is dropped.
  const latest = useRef(workspace);
  latest.current = workspace;

  const load = useCallback(async () => {
    setLoading(true);
    const r = await commands
      .nearbyPrs(workspace)
      .catch((e): CommandResult<NearbyPrsDto> => ({ status: "error", error: errorText(e) }));
    if (latest.current !== workspace) return;
    setLoading(false);
    if (r.status === "error") {
      setPrs(undefined);
      setNote(quiet(r.error));
      return;
    }
    setPrs(r.data.prs);
    setNote(emptyNote(r.data));
  }, [workspace]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: the selection is per workspace
  useEffect(() => setSelected(undefined), [workspace]);

  useEffect(() => {
    if (!githubOff) load();
  }, [githubOff, load]);

  function preview(pr: NearbyPrDto) {
    setSelected(pr.number);
    const api = dockOf(workspace);
    if (api) openPrPreview(api, pr);
  }

  async function open(pr: NearbyPrDto) {
    setSelected(pr.number);
    setOpening(pr.number);
    const r = await commands.openPr(pr.url);
    setOpening(undefined);
    if (r.status === "error") return onError(r.error);
    const api = dockOf(workspace);
    if (api) closePrPreview(api, pr.url);
    onOpened(r.data.workspace, r.data.setup);
  }

  return (
    <div className="nearby-prs">
      <div className="workspaces-bar">
        Pull requests
        <span className="spacer" />
        <button
          type="button"
          onClick={load}
          disabled={loading || githubOff}
          title="Find open PRs of bookmarks pushed from this workspace's stack"
        >
          {loading ? "Loading" : "Refresh"}
        </button>
      </div>
      {githubOff ? (
        <div className="nearby-note">Needs the GitHub CLI (gh)</div>
      ) : (
        note && <div className="nearby-note">{note}</div>
      )}
      <ul>
        {prs?.map((pr) => (
          <li key={pr.number} className={`workspace${selected === pr.number ? " selected" : ""}`}>
            <button
              type="button"
              className="workspace-open"
              onClick={() => preview(pr)}
              onDoubleClick={() => open(pr)}
              onKeyDown={(e) => {
                if (e.key !== "Enter") return;
                e.preventDefault();
                open(pr);
              }}
              disabled={opening !== undefined}
              title={`${pr.url}\nDouble-click to review it in its own workspace`}
            >
              <span className="workspace-title">
                #{pr.number} {pr.title}
              </span>
              <span className="workspace-desc">
                {opening === pr.number
                  ? "Fetching…"
                  : `${pr.bookmark} · ${pr.author}${pr.draft ? " · draft" : ""}`}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
