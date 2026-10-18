import { useEffect, useMemo, useRef, useState } from "react";
import type { CommentDto, ReviewDto, Stage, StateDto } from "../bindings";
import { type CommandResult, commands } from "../ipc";
import { confirmAction } from "../ui/prompt";
import type { WorkspaceName } from "../workspaces/context";
import { reviewText, triageText } from "./agentText";
import { commentText, type Draft, followDraft } from "./comments";
import type { Moves } from "./follow";
import { sendToAgent, talk as talkTo } from "./linkCommands";
import { type FlatFile, NEW_COMMENT } from "./model";
import { prComments } from "./prComments";

const NONE: CommentDto[] = [];

const without = <V>(m: ReadonlyMap<number, V>, id: number) => {
  const next = new Map(m);
  next.delete(id);
  return next;
};

const withEntry = <V>(m: ReadonlyMap<number, V>, id: number, v: V) => new Map(m).set(id, v);

const withoutId = (s: ReadonlySet<number>, id: number) => {
  const next = new Set(s);
  next.delete(id);
  return next;
};

const moved = <V>(m: ReadonlyMap<number, V>, from: number, to: number) => {
  const v = m.get(from);
  return v === undefined ? m : without(m, from).set(to, v);
};

interface Deps {
  review: ReviewDto | undefined;
  files: FlatFile[];
  /** Where rows went when `files` last changed. */
  moves: Moves | undefined;
  /** Shows a new state after a comment on `review` changed. */
  onState(state: StateDto, review?: number): void;
  onError(message: string): void;
  workspace: WorkspaceName;
  /** Pastes text into terminal `preferred` when it can take it, else one of the workspace's
   * terminals, then calls `onSent` with it. */
  send(
    text: string,
    onSent?: (terminal: string) => void,
    report?: undefined,
    preferred?: string,
  ): void;
  /** Called when the user is done in a comment block, to give keys back to the review. */
  onDone(): void;
  /** The review is in a PR workspace, so drafts for GitHub can be written. */
  canDraft: boolean;
}

/** The comment being drafted and bodies being edited. */
export function useComments({
  review,
  files,
  moves,
  onState,
  onError,
  workspace,
  send,
  onDone,
  canDraft,
}: Deps) {
  const [draft, setDraft] = useState<Draft>();
  /** Bodies being edited, by comment id. Present means the block is in edit mode. */
  const [edits, setEdits] = useState<ReadonlyMap<number, string>>(new Map());
  /** Threads being sent to GitHub. */
  const [sending, setSending] = useState<ReadonlySet<number>>(new Set());
  /** The block whose text box should take focus when it next renders. */
  const [focus, setFocus] = useState<number>();
  const comments = review?.state.comments ?? NONE;
  const threads = review?.state.threads;
  /** Comments on the PR itself and their quotes, and threads whose file isn't in the review,
   * which show above the files. */
  const onPr = useMemo(() => prComments(threads ?? [], comments), [threads, comments]);

  // The draft follows its rows when the diff changes, and goes when they do.
  const seen = useRef(files);
  // biome-ignore lint/correctness/useExhaustiveDependencies: only when the diff changes
  useEffect(() => {
    if (seen.current === files) return;
    seen.current = files;
    if (!draft) return;
    const next = followDraft(draft, moves);
    setDraft(next);
    if (!next) setEdits((m) => without(m, NEW_COMMENT));
  }, [files, moves]);

  async function apply(result: Promise<CommandResult<StateDto>>): Promise<StateDto | undefined> {
    const id = review?.id;
    const r = await result;
    if (r.status === "error") {
      onError(r.error);
      return undefined;
    }
    onState(r.data, id);
    return r.data;
  }

  /** Opens a draft with the cursor in its body. */
  function start(next: Draft) {
    setDraft(next);
    setEdits((m) => withEntry(m, NEW_COMMENT, m.get(NEW_COMMENT) ?? ""));
    setFocus(NEW_COMMENT);
  }

  /** Saves the draft and returns the new comment. */
  async function create(body: string): Promise<CommentDto | undefined> {
    if (!review || !draft) return undefined;
    const before = new Set(comments.map((c) => c.id));
    const rows = draft.rows.map((row) => ({ file: draft.file, row }));
    const state = await apply(
      commands.addComment(review.id, rows, body, draft.snippet, !!draft.github),
    );
    const saved = state?.comments.find((c) => !before.has(c.id));
    if (!saved) return undefined;
    const id = saved.id;
    setDraft(undefined);
    setEdits((m) => moved(m, NEW_COMMENT, id));
    return saved;
  }

  async function save(id: number) {
    if (!review) return;
    const body = edits.get(id) ?? "";
    const saved =
      id === NEW_COMMENT
        ? (await create(body))?.id
        : (await apply(commands.editComment(review.id, id, body))) && id;
    if (saved === undefined) return;
    setEdits((m) => without(m, saved));
    onDone();
  }

  function cancel(id: number) {
    setEdits((m) => without(m, id));
    if (id === NEW_COMMENT) setDraft(undefined);
    onDone();
  }

  async function remove(id: number) {
    if (review && (await apply(commands.deleteComment(review.id, id)))) {
      setEdits((m) => without(m, id));
      onDone();
    }
  }

  /** Opens the comment's text for editing. */
  function edit(c: CommentDto) {
    setEdits((m) => withEntry(m, c.id, c.body));
    setFocus(c.id);
  }

  /** Saves the block's text when it's being edited. Resolves to the comment's id. */
  async function saveEdit(id: number): Promise<number | undefined> {
    const editing = edits.get(id);
    if (!review || editing === undefined) return id;
    const cid =
      id === NEW_COMMENT
        ? (await create(editing))?.id
        : (await apply(commands.editComment(review.id, id, editing))) && id;
    if (cid === undefined) return undefined;
    setEdits((m) => without(m, cid));
    return cid;
  }

  /** Saves the thread's reply as final, which makes it ready to send. */
  async function final(id: number) {
    const cid = await saveEdit(id);
    if (review && cid !== undefined) await apply(commands.setThreadStage(review.id, cid, "ready"));
    onDone();
  }

  /** Sends a final thread to GitHub on its own. */
  async function sendOne(id: number) {
    if (!review) return;
    setSending((s) => new Set(s).add(id));
    await apply(commands.sendThread(review.id, id));
    setSending((s) => withoutId(s, id));
  }

  async function setStage(id: number, stage: Stage) {
    if (review) await apply(commands.setThreadStage(review.id, id, stage));
  }

  /** Clears a GitHub thread's take, reply and conversation, after asking. */
  async function reset(id: number) {
    if (!review) return;
    if (!(await confirmAction("Reset this thread? Its take and reply go.", "Reset"))) return;
    setEdits((m) => without(m, id));
    await apply(commands.resetThread(review.id, id));
  }

  async function setDone(id: number, done: boolean) {
    if (review) await apply(commands.setNoteDone(review.id, id, done));
  }

  /** Starts a GitHub draft on the note's code, with its text. */
  async function draftFromNote(id: number) {
    if (review) await apply(commands.draftFromNote(review.id, id));
  }

  function sendComment(c: CommentDto) {
    send(commentText(files, c), onDone);
  }

  /** Makes the thread the one I'm on and focuses the linked agent's terminal. */
  function talk(c: CommentDto) {
    talkTo(workspace, c.id, onError);
  }

  /** Pastes a new thread and my rules for replies into the linked or send target terminal,
   * for its agent to make the first take and reply. */
  async function triage(c: CommentDto) {
    const rules = await commands.promptText("reply");
    if (rules.status === "error") return onError(rules.error);
    sendToAgent(workspace, send, triageText(files, c, rules.data), c.id);
  }

  /** Pastes a thread and my rules for replies into the linked or send target terminal, for its
   * agent to review the reply and suggest a better one. */
  async function reviewReply(c: CommentDto) {
    const rules = await commands.promptText("reply");
    if (rules.status === "error") return onError(rules.error);
    sendToAgent(workspace, send, reviewText(files, c, rules.data), c.id);
  }

  /** Makes a thread's suggested reply its reply. */
  async function takeSuggestion(id: number) {
    if (!review) return;
    setEdits((m) => without(m, id));
    await apply(commands.takeSuggestion(review.id, id));
  }

  async function dismissSuggestion(id: number) {
    if (review) await apply(commands.dismissSuggestion(review.id, id));
  }

  /** Starts a thread on `text`, quoted from comment `of` on the PR. */
  async function quote(of: number, text: string) {
    if (review && text.trim()) await apply(commands.addQuote(review.id, of, text));
  }

  /** Posts `body` as the reply of quotes `ids`. Resolves to whether it went. */
  async function sendCombined(ids: number[], body: string) {
    if (!review) return false;
    return !!(await apply(commands.sendCombined(review.id, ids, body)));
  }

  return {
    reviewId: review?.id,
    draft,
    edits,
    focus,
    comments,
    onPr,
    quote,
    sendCombined,
    start,
    setBody: (id: number, text: string) => setEdits((m) => withEntry(m, id, text)),
    focused: () => setFocus(undefined),
    save,
    cancel,
    edit,
    remove,
    send: sendComment,
    workspace,
    talk,
    reviewReply,
    triage,
    takeSuggestion,
    dismissSuggestion,
    final,
    sending,
    sendOne,
    setStage,
    reset,
    setDone,
    draftFromNote,
    canDraft,
    done: onDone,
  };
}

export type Comments = ReturnType<typeof useComments>;
