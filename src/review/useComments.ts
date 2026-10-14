import { Channel } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { endFrom, startRun } from "../agents/runs";
import type { CommentDto, ReviewDto, Stage, StateDto } from "../bindings";
import { type CommandResult, commands } from "../ipc";
import { confirmAction } from "../ui/prompt";
import { useWorkspace } from "../workspaces/context";
import { commentText, type Draft, followDraft } from "./comments";
import { currentReview } from "./current";
import type { Moves } from "./follow";
import { type FlatFile, NEW_COMMENT } from "./model";

/** A workshop exchange in flight: what was asked and the reply so far. */
export interface Running {
  ask: string;
  change: boolean;
  /** A first take and reply, asked for with no message. */
  compose?: boolean;
  reply: string;
}

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

interface Focus {
  id: number;
  field: "body" | "ask";
}

interface Deps {
  review: ReviewDto | undefined;
  files: FlatFile[];
  /** Where rows went when `files` last changed. */
  moves: Moves | undefined;
  /** Shows a new state after a comment on `review` changed. */
  onState(state: StateDto, review?: number): void;
  onError(message: string): void;
  /** Pastes text into one of the workspace's terminals, then calls `onSent`. */
  send(text: string, onSent: () => void): void;
  /** Called when the user is done in a comment block, to give keys back to the review. */
  onDone(): void;
  /** Called after the agent was asked to change the code, to show its edits. */
  onChanged(): void;
  /** The review is in a PR workspace, so drafts for GitHub can be written. */
  canDraft: boolean;
}

/** The comment being drafted, bodies being edited, and workshop conversations. */
export function useComments({
  review,
  files,
  moves,
  onState,
  onError,
  send,
  onDone,
  onChanged,
  canDraft,
}: Deps) {
  const workspace = useWorkspace();
  const [draft, setDraft] = useState<Draft>();
  /** Bodies being edited, by comment id. Present means the block is in edit mode. */
  const [edits, setEdits] = useState<ReadonlyMap<number, string>>(new Map());
  /** Unsent workshop messages, by comment id. */
  const [asks, setAsks] = useState<ReadonlyMap<number, string>>(new Map());
  const [running, setRunning] = useState<ReadonlyMap<number, Running>>(new Map());
  /** Blocks whose ask box was opened. It also shows while a block has a conversation. */
  const [askOpen, setAskOpen] = useState<ReadonlySet<number>>(new Set());
  /** The text box that should take focus when its block next renders. */
  const [focus, setFocus] = useState<Focus>();
  const comments = review?.state.comments ?? NONE;
  const byId = (id: number) => comments.find((c) => c.id === id);

  // The draft follows its rows when the diff changes, and goes when they do.
  const seen = useRef(files);
  // biome-ignore lint/correctness/useExhaustiveDependencies: only when the diff changes
  useEffect(() => {
    if (seen.current === files) return;
    seen.current = files;
    if (!draft) return;
    const next = followDraft(draft, moves);
    setDraft(next);
    if (!next) {
      setEdits((m) => without(m, NEW_COMMENT));
      setAsks((m) => without(m, NEW_COMMENT));
      setAskOpen((s) => withoutId(s, NEW_COMMENT));
    }
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

  /** Opens a draft with the cursor in its body, or in its ask box to request a change. */
  function start(next: Draft, field: Focus["field"] = "body") {
    setDraft(next);
    setEdits((m) => withEntry(m, NEW_COMMENT, m.get(NEW_COMMENT) ?? ""));
    if (field === "ask") setAskOpen((s) => new Set(s).add(NEW_COMMENT));
    setFocus({ id: NEW_COMMENT, field });
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
    setAsks((m) => moved(m, NEW_COMMENT, id));
    setAskOpen((s) => (s.has(NEW_COMMENT) ? withoutId(s, NEW_COMMENT).add(id) : s));
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
    if (id === NEW_COMMENT) {
      setDraft(undefined);
      setAsks((m) => without(m, id));
      setAskOpen((s) => withoutId(s, id));
    }
    onDone();
  }

  async function remove(id: number) {
    if (review && (await apply(commands.deleteComment(review.id, id)))) {
      setEdits((m) => without(m, id));
      onDone();
    }
  }

  /** Sends the block's workshop message, saving a draft first so the exchange has a home.
   * A change with no message asks for what the comment says. */
  async function ask(id: number, change: boolean, compose = false) {
    const message = compose ? "" : (asks.get(id)?.trim() ?? "");
    const body = edits.get(id) ?? byId(id)?.body ?? "";
    if (!review || running.has(id) || !(compose || message || (change && body.trim()))) return;
    const c = id === NEW_COMMENT ? await create(body) : byId(id);
    if (!c) return;
    const cid = c.id;
    // A saved draft's block is a new one, so the ask box needs focus again.
    if (cid !== id) setFocus({ id: cid, field: "ask" });
    if (!compose) setAsks((m) => without(m, cid));
    setRunning((m) => withEntry(m, cid, { ask: message, change, compose, reply: "" }));
    const handle = startRun(workspace, {
      kind: change ? "change" : c.thread ? "reply" : "ask",
      subject: c.path || "Comment",
      stop: () => commands.stopWorkshop(cid),
      open: () => currentReview(workspace)?.jumpToComment(cid),
    });
    const channel = new Channel<string>();
    channel.onmessage = (text) =>
      setRunning((m) => {
        const r = m.get(cid);
        return r ? withEntry(m, cid, { ...r, reply: r.reply + text }) : m;
      });
    const r = await commands.workshopComment(
      review.id,
      cid,
      { message, draft: body, change, compose },
      channel,
    );
    endFrom(handle, r);
    await apply(Promise.resolve(r));
    setRunning((m) => without(m, cid));
    if (change && r.status === "ok") onChanged();
  }

  /** Puts an agent's suggestion in the body for the user to check and save. */
  function use(id: number, text: string) {
    setEdits((m) => withEntry(m, id, text));
    setFocus({ id, field: "body" });
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

  async function setStage(id: number, stage: Stage) {
    if (review) await apply(commands.setThreadStage(review.id, id, stage));
  }

  /** Clears a GitHub thread's take, reply and agent conversation, after asking. */
  async function reset(id: number) {
    if (!review) return;
    if (!(await confirmAction("Reset this thread? Its take, reply and agent chat go.", "Reset")))
      return;
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

  /** Has the agent write a thread's first take and reply. */
  const compose = (id: number) => ask(id, false, true);

  /** The `a` key on a block: drafts a thread's reply when it has neither take nor reply, and
   * otherwise puts the cursor in the ask box. */
  function agentKey(id: number) {
    const c = byId(id);
    if (c?.github && c.thread && !c.thread.take && !c.body.trim() && !running.has(id)) {
      compose(id);
    } else openAsk(id);
  }

  /** Shows the block's ask box with the cursor in it. */
  function openAsk(id: number) {
    setAskOpen((s) => new Set(s).add(id));
    setFocus({ id, field: "ask" });
  }

  /** Hides the block's ask box, unless it holds a conversation, and gives keys back. */
  function closeAsk(id: number) {
    setAskOpen((s) => withoutId(s, id));
    onDone();
  }

  function sendComment(c: CommentDto) {
    send(commentText(files, c), onDone);
  }

  return {
    draft,
    edits,
    asks,
    running,
    focus,
    comments,
    start,
    setBody: (id: number, text: string) => setEdits((m) => withEntry(m, id, text)),
    setAsk: (id: number, text: string) => setAsks((m) => withEntry(m, id, text)),
    focused: () => setFocus(undefined),
    save,
    cancel,
    edit: (c: CommentDto) => use(c.id, c.body),
    remove,
    ask,
    compose,
    agentKey,
    stop: commands.stopWorkshop,
    use,
    send: sendComment,
    final,
    setStage,
    reset,
    setDone,
    draftFromNote,
    /** Whether the block's ask box shows: opened, or holding a conversation. */
    asking: (id: number) => askOpen.has(id) || running.has(id) || !!byId(id)?.messages.length,
    openAsk,
    closeAsk,
    canDraft,
    /** The agent edits `@`, so a change only shows when the review includes it. */
    canChange: review?.hasWorkingCopy ?? false,
    done: onDone,
  };
}

export type Comments = ReturnType<typeof useComments>;
