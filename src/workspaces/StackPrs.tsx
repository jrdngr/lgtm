import { useCallback, useEffect, useRef, useState } from "react";
import type { NearbyPrDto } from "../bindings";
import { dockOf } from "../core/docks";
import type { WorkspaceName } from "../core/workspace";
import { prOf, usePr } from "../github/pr";
import { openPrPreview } from "../github/preview";
import { type CommandResult, commands, errorText, unwrap } from "../ipc";
import { openUrl } from "../services/pathMenu";
import { useGithubOff } from "../tools/agents";
import { type CtxEntry, copy, openMenu } from "../ui/contextMenu";
import { clearStackPr, showStackPr } from "./stack";
import { useRepoChanged } from "./useRepoChanged";

interface Props {
  /** The workspace whose `@` the PRs are found from. */
  workspace: WorkspaceName;
  /** Changes when the user asks for a refresh, which asks GitHub again. */
  refreshed: number;
  onError(message: string | undefined): void;
}

/** A gh or remote problem as a quiet note: listing PRs is a convenience. */
function quiet(error: string): string | undefined {
  if (/auth login|not logged in/i.test(error)) return "Sign in with `gh auth login` to list PRs";
  if (/no GitHub remote/i.test(error)) return undefined;
  return error;
}

/** The open PRs of the bookmarks on the selected workspace's stack, under its row. The radio
 * picks one as the workspace's PR. One `@` doesn't build on asks to edit its change first.
 * Following `@` keeps the pick in line with the stack as the repo changes. */
export function StackPrs({ workspace, refreshed, onError }: Props) {
  const githubOff = useGithubOff();
  const selected = usePr(workspace);
  const [prs, setPrs] = useState<NearbyPrDto[]>();
  const [note, setNote] = useState<string>();
  const [asking, setAsking] = useState<NearbyPrDto>();
  const [busy, setBusy] = useState(false);

  // The workspace the latest load is for: a reply for an earlier one is dropped.
  const latest = useRef(workspace);
  latest.current = workspace;

  const load = useCallback(
    async (refresh = false) => {
      const before = await prOf(workspace);
      const synced = await commands.syncStackPr(workspace);
      if (latest.current !== workspace) return;
      if (synced.status === "ok") showStackPr(workspace, before, synced.data);
      const r = await commands.nearbyPrs(workspace, refresh).catch(
        (e): CommandResult<{ prs: NearbyPrDto[] }> => ({
          status: "error",
          error: errorText(e),
        }),
      );
      if (latest.current !== workspace) return;
      if (r.status === "error") {
        setPrs(undefined);
        setNote(quiet(r.error));
        return;
      }
      setPrs(r.data.prs);
      setNote(undefined);
    },
    [workspace],
  );

  // biome-ignore lint/correctness/useExhaustiveDependencies: the question is per workspace
  useEffect(() => setAsking(undefined), [workspace]);

  useEffect(() => {
    if (!githubOff) load();
  }, [githubOff, load]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs on each refresh
  useEffect(() => {
    if (refreshed && !githubOff) load(true);
  }, [refreshed]);
  useRepoChanged(() => {
    if (!githubOff) load();
  });

  async function run(f: () => Promise<void>) {
    setBusy(true);
    onError(undefined);
    await f().catch((e) => onError(errorText(e)));
    setBusy(false);
  }

  function pick(pr: NearbyPrDto) {
    setAsking(undefined);
    if (selected?.stack && selected.number === pr.number) {
      run(() => clearStackPr(workspace, selected));
    } else if (!pr.under) {
      setAsking(pr);
    } else {
      run(() => select(pr, false));
    }
  }

  async function select(pr: NearbyPrDto, edit: boolean) {
    const before = await prOf(workspace);
    const next = await unwrap(commands.selectStackPr(workspace, pr.number, edit));
    showStackPr(workspace, before, next);
  }

  function prMenu(e: React.MouseEvent, pr: NearbyPrDto) {
    const api = dockOf(workspace);
    openMenu(e, [
      { label: "Preview", disabled: !api, onSelect: () => api && openPrPreview(api, pr) },
      "-",
      { label: "Open on GitHub", onSelect: () => openUrl(pr.url) },
      { label: "Copy PR URL", onSelect: () => copy(pr.url) },
      { label: "Copy Bookmark Name", onSelect: () => copy(pr.bookmark) },
      "-",
      refresh,
    ]);
  }

  const refresh: CtxEntry = {
    label: "Refresh List",
    disabled: githubOff,
    onSelect: () => load(true),
  };

  if (githubOff || (selected && !selected.stack)) return null;
  if (!prs?.length) return note ? <li className="stack-note">{note}</li> : null;
  return (
    <li className="stack-prs" onContextMenu={(e) => openMenu(e, [refresh])}>
      <div className="stack-cap">PRs in this stack</div>
      {prs.map((pr) => {
        const on = !!selected?.stack && selected.number === pr.number;
        return (
          <button
            type="button"
            key={pr.number}
            className={`stack-pr${on ? " on" : ""}`}
            disabled={busy}
            onClick={() => pick(pr)}
            onContextMenu={(e) => {
              e.stopPropagation();
              prMenu(e, pr);
            }}
            title={`${pr.title}\n${pr.url}`}
          >
            <span className="stack-radio" />
            <span className="stack-lines">
              <span className="stack-l1">
                <span className="stack-num">#{pr.number}</span>
                <span className="ellipsis">{pr.title}</span>
                {pr.draft && <span className="pill">draft</span>}
                <span className={`stack-pos${pr.position === "@" ? " here" : ""}`}>
                  {pr.position}
                </span>
              </span>
              <span className="stack-bm">{pr.bookmark}</span>
            </span>
          </button>
        );
      })}
      {asking && (
        <div className="stack-ask">
          <span>
            Edit <code>{asking.change}</code> to select #{asking.number}?
          </span>
          <div className="stack-ask-btns">
            <button type="button" onClick={() => setAsking(undefined)}>
              Cancel
            </button>
            <button
              type="button"
              className="primary"
              // biome-ignore lint/a11y/noAutofocus: the question takes Enter right away
              autoFocus
              onClick={() => {
                setAsking(undefined);
                run(() => select(asking, true));
              }}
            >
              Edit
            </button>
          </div>
        </div>
      )}
    </li>
  );
}
