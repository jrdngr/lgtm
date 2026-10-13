import type { IDockviewPanelProps } from "dockview-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { PrDto, Stage, StateDto, ThreadDto } from "../bindings";
import { Inline } from "../bots/Markdown";
import { Avatar } from "../github/Post";
import { viewerLogin } from "../github/people";
import { usePr } from "../github/pr";
import { useDiscussion } from "../github/useDiscussion";
import { type CommandResult, commands } from "../ipc";
import { type CurrentReview, useCurrentReview } from "../review/current";
import { openSettings } from "../settings/open";
import { useAgentsOff } from "../tools/agents";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { isMac } from "../workbench/keys";
import { useWorkspace } from "../workspaces/context";
import { FocusedThread } from "./FocusedThread";
import { useFocusRequests } from "./focus";
import { SendBox } from "./SendBox";
import { StageBar } from "./StageBar";
import { byStage, STAGES, threadLabel, threadWhere } from "./stages";
import { TakePill } from "./ThreadBits";
import { finalText, threadText } from "./text";
import { useThreadChat } from "./useThreadChat";
import { useThreadRun } from "./useThreadRun";
import "../review/comments.css";
import "./threads.css";

const isEditable = (t: EventTarget) =>
  t instanceof HTMLInputElement ||
  t instanceof HTMLSelectElement ||
  t instanceof HTMLTextAreaElement;

function Row({ t, focused, onClick }: { t: ThreadDto; focused: boolean; onClick(): void }) {
  const last = t.github?.posts.at(-1);
  return (
    <button
      type="button"
      className="threads-row"
      aria-current={focused}
      data-thread={t.id}
      onClick={onClick}
    >
      <Avatar name={last?.author ?? "you"} small />
      <span className="threads-summary">
        {!t.github && <span className="pill purple">Draft</span>} {threadLabel(t)}
      </span>
      <span className="threads-where">{threadWhere(t)}</span>
      <span className="threads-latest">
        {last && t.state.summary && <Inline>{last.body.split("\n")[0]}</Inline>}
      </span>
      <TakePill take={t.state.take} />
    </button>
  );
}

interface BodyProps {
  pr: PrDto;
  review: CurrentReview;
  api: IDockviewPanelProps["api"];
}

function ThreadsBody({ pr, review, api }: BodyProps) {
  const workspace = useWorkspace();
  const discussion = useDiscussion(workspace, pr);
  const viewer = discussion.d?.viewer ?? viewerLogin(review.comments);
  const [error, setError] = useState<string>();
  const chat = useThreadChat(review, setError);
  const runner = useThreadRun(review, setError);
  const agentsOff = useAgentsOff();
  const [focus, setFocus] = useState<number>();
  const [showDone, setShowDone] = useState(false);
  const [confirm, setConfirm] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const replyRef = useRef<HTMLTextAreaElement>(null);
  const askRef = useRef<HTMLTextAreaElement>(null);
  const threads = review.threads;
  const groups = byStage(threads);
  const visible = groups.flatMap(([s, ts]) => (s === "done" && !showDone ? [] : ts));
  const current = threads.find((t) => t.id === focus) ?? visible[0];
  const open = threads.filter((t) => t.state.stage !== "done").length;

  const onFocusRequest = useCallback((id: number) => {
    setFocus(id);
    setShowDone(true);
  }, []);
  useFocusRequests(workspace, onFocusRequest);

  // Keys go to the list as soon as the tab is picked.
  useEffect(() => {
    if (api.isActive) rootRef.current?.focus({ preventScroll: true });
    const sub = api.onDidActiveChange(({ isActive }) => {
      if (isActive) rootRef.current?.focus({ preventScroll: true });
    });
    return () => sub.dispose();
  }, [api]);

  // The focused row stays in view as it moves between sections.
  useEffect(() => {
    if (current === undefined) return;
    rootRef.current
      ?.querySelector(`[data-thread="${current.id}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [current]);

  async function apply(r: Promise<CommandResult<StateDto>>) {
    const got = await r;
    if (got.status === "error") return setError(got.error);
    setError(undefined);
    review.apply(got.data);
  }

  const stage = (t: ThreadDto, s: Stage) => apply(commands.setThreadStage(review.id, t.id, s));
  const leave = () => rootRef.current?.focus({ preventScroll: true });

  function move(dir: 1 | -1) {
    const i = current ? visible.indexOf(current) : -1;
    const next = visible[Math.min(Math.max(i + dir, 0), visible.length - 1)];
    if (next) setFocus(next.id);
  }

  function onKey(e: React.KeyboardEvent) {
    if (isEditable(e.target)) return;
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (mod && e.key === "Enter") setConfirm((n) => n + 1);
    else if (mod) return;
    else if (e.key === "t" && !agentsOff) runner.start("triage");
    else if (e.key === "j" || e.key === "ArrowDown") move(1);
    else if (e.key === "k" || e.key === "ArrowUp") move(-1);
    else if (!current) return;
    else if (e.key === "f") stage(current, "toFix");
    else if (e.key === "r") {
      stage(current, "reply");
      askRef.current?.focus();
    } else if (e.key === "e") replyRef.current?.focus();
    else if (e.key === "y") stage(current, "ready");
    else if (e.key === "d") stage(current, "done");
    else if (e.key === "o" && current.placed) review.jumpToComment(current.id);
    else return;
    e.preventDefault();
  }

  const toFix = threads.filter((t) => t.state.stage === "toFix");
  const fresh = threads.filter((t) => t.state.stage === "new").length;
  const run = runner.run;

  return (
    <div ref={rootRef} className="threads" tabIndex={-1} onKeyDown={onKey}>
      <div className="threads-head">
        <span className="threads-title">Threads</span>
        <span className="pr-number">#{pr.number}</span>
        <span className="threads-pr-title">{pr.title}</span>
        <span className="spacer" />
        <span className="faint">
          {open} open · {threads.length - open} done
        </span>
        {!agentsOff && fresh > 0 && !run && (
          <button
            type="button"
            className="primary"
            onClick={() => runner.start("triage")}
            title="An agent reads each new thread's code and drafts a take and reply (t)"
          >
            ✦ Triage {fresh} new
          </button>
        )}
        <IconButton
          icon="sync"
          label="Sync with GitHub"
          onClick={() => {
            review.refetchPr();
            discussion.load();
          }}
        />
        <Menu
          items={[
            { label: showDone ? "Hide done" : "Show done", onSelect: () => setShowDone(!showDone) },
            {
              label: "Copy final replies",
              title: "Final replies, threads fixed as asked, and what's still open, as text",
              onSelect: () => navigator.clipboard.writeText(finalText(threads)),
            },
            {
              label: "Edit reply prompt…",
              title: "The rules the agent follows for takes and replies",
              onSelect: () => openSettings(workspace, "prompts"),
            },
          ]}
        />
      </div>
      <div className="threads-bar">
        <StageBar threads={threads} />
      </div>
      {run && (
        <div className="threads-run">
          <span className="spinner" />
          <span>
            {run.kind === "triage" ? "Triaging" : "Fixing"}
            {run.activity && ` · ${run.activity}`}
          </span>
          <span className="spacer" />
          <button type="button" onClick={runner.stop}>
            Stop
          </button>
        </div>
      )}
      {error && <div className="review-error">{error}</div>}
      <div className="threads-list">
        {threads.length === 0 && (
          <div className="placeholder">
            No threads yet. Sync, or select code in the review and press <kbd>C</kbd>
          </div>
        )}
        {groups.map(([s, ts]) => {
          const [, name, tone] = STAGES.find(([k]) => k === s) ?? [];
          const shut = s === "done" && !showDone;
          return (
            <section key={s} className="threads-section">
              <div className="threads-section-head">
                {s === "done" ? (
                  <button type="button" className="quiet" onClick={() => setShowDone(!showDone)}>
                    {shut ? "▸" : "▾"}
                  </button>
                ) : null}
                <span className={`dot stage-seg ${tone || "grey"}`} />
                <b>{name}</b>
                <span className="faint">{ts.length}</span>
                <span className="spacer" />
                {s === "toFix" && !agentsOff && (
                  <button
                    type="button"
                    className="primary"
                    onClick={() => runner.start("fix")}
                    disabled={!!run}
                    title="An agent changes the code in @ for these threads"
                  >
                    Fix {toFix.length}
                  </button>
                )}
                {s === "toFix" && (
                  <button
                    type="button"
                    onClick={() => review.send(toFix.map(threadText).join("\n\n---\n\n"))}
                    title="Paste these threads into a terminal's prompt"
                  >
                    Send to terminal
                  </button>
                )}
              </div>
              {!shut &&
                ts.map((t) => (
                  <div key={t.id}>
                    <Row
                      t={t}
                      focused={t.id === current?.id}
                      onClick={() => {
                        setFocus(t.id);
                        leave();
                      }}
                    />
                    {t.id === current?.id && (
                      <FocusedThread
                        thread={t}
                        review={review}
                        viewer={viewer}
                        chat={chat}
                        replyRef={replyRef}
                        askRef={askRef}
                        onLeave={leave}
                        onError={setError}
                      />
                    )}
                  </div>
                ))}
            </section>
          );
        })}
      </div>
      <SendBox
        review={review}
        threads={threads}
        repo={pr.repo}
        own={!!viewer && pr.author === viewer}
        onSent={discussion.load}
        confirm={confirm}
      />
    </div>
  );
}

/** The workspace's PR threads as a list to work down, with the one at the cursor open in
 * place. Finished threads fold into Done at the end. */
export function ThreadsPanel({ api }: IDockviewPanelProps) {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const pr = usePr(workspace);
  useEffect(() => {
    api.setTitle(pr ? `Threads #${pr.number}` : "Threads");
  }, [api, pr]);
  if (pr === null) return <div className="placeholder">This workspace isn't a PR's</div>;
  if (!pr || !review) return <div className="placeholder">Open a review to see its threads</div>;
  return <ThreadsBody pr={pr} review={review} api={api} />;
}
