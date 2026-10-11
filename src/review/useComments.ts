import { Channel } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import type { CommentDto, ReviewDto, StateDto } from "../bindings";
import { type CommandResult, commands } from "../ipc";
import { commentText, type Draft, followDraft } from "./comments";
import type { Moves } from "./follow";
import { type FlatFile, NEW_COMMENT } from "./model";

/** A workshop exchange in flight: what was asked and the reply so far. */
export interface Running {
  ask: string;
  change: boolean;
  reply: string;
}

const NONE: CommentDto[] = [];

const without = <V>(m: ReadonlyMap<number, V>, id: number) => {
  const next = new Map(m);
  next.delete(id);
  return next;
};

const withEntry = <V>(m: ReadonlyMap<number, V>, id: number, v: V) => new Map(m).set(id, v);

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
  /** Shows a new state after a comment changed. */
  onState(state: StateDto): void;
  onError(message: string): void;
  /** Pastes text into one of the workspace's terminals, then calls `onSent`. */
  send(text: string, onSent: () => void): void;
  /** Called when the user is done in a comment block, to give keys back to the review. */
  onDone(): void;
  /** Called after the agent was asked to change the code, to show its edits. */
  onChanged(): void;
  /** The review is a PR's, so comments can be published to it. */
  canPublish: boolean;
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
  canPublish,
}: Deps) {
  const [draft, setDraft] = useState<Draft>();
  /** Bodies being edited, by comment id. Present means the block is in edit mode. */
  const [edits, setEdits] = useState<ReadonlyMap<number, string>>(new Map());
  /** Unsent workshop messages, by comment id. */
  const [asks, setAsks] = useState<ReadonlyMap<number, string>>(new Map());
  const [running, setRunning] = useState<ReadonlyMap<number, Running>>(new Map());
  const [publishing, setPublishing] = useState<ReadonlySet<number>>(new Set());
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
    }
  }, [files, moves]);

  async function apply(result: Promise<CommandResult<StateDto>>): Promise<StateDto | undefined> {
    const r = await result;
    if (r.status === "error") {
      onError(r.error);
      return undefined;
    }
    onState(r.data);
    return r.data;
  }

  /** Opens a draft with the cursor in its body, or in its ask box to request a change. */
  function start(next: Draft, field: Focus["field"] = "body") {
    setDraft(next);
    setEdits((m) => withEntry(m, NEW_COMMENT, m.get(NEW_COMMENT) ?? ""));
    setFocus({ id: NEW_COMMENT, field });
  }

  /** Saves the draft and returns its new id. */
  async function create(body: string): Promise<number | undefined> {
    if (!review || !draft) return undefined;
    const before = new Set(comments.map((c) => c.id));
    const rows = draft.rows.map((row) => ({ file: draft.file, row }));
    const state = await apply(commands.addComment(review.id, rows, body, draft.snippet));
    const id = state?.comments.find((c) => !before.has(c.id))?.id;
    if (id === undefined) return undefined;
    setDraft(undefined);
    setEdits((m) => moved(m, NEW_COMMENT, id));
    setAsks((m) => moved(m, NEW_COMMENT, id));
    return id;
  }

  async function save(id: number) {
    if (!review) return;
    const body = edits.get(id) ?? "";
    const saved =
      id === NEW_COMMENT
        ? await create(body)
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
  async function ask(id: number, change: boolean) {
    const message = asks.get(id)?.trim() ?? "";
    const body = edits.get(id) ?? byId(id)?.body ?? "";
    if (!review || running.has(id) || !(message || (change && body.trim()))) return;
    const cid = id === NEW_COMMENT ? await create(body) : id;
    if (cid === undefined) return;
    // A saved draft's block is a new one, so the ask box needs focus again.
    if (cid !== id) setFocus({ id: cid, field: "ask" });
    setAsks((m) => without(m, cid));
    setRunning((m) => withEntry(m, cid, { ask: message, change, reply: "" }));
    const channel = new Channel<string>();
    channel.onmessage = (text) =>
      setRunning((m) => {
        const r = m.get(cid);
        return r ? withEntry(m, cid, { ...r, reply: r.reply + text }) : m;
      });
    await apply(
      commands.workshopComment(review.id, cid, { message, draft: body, change }, channel),
    );
    setRunning((m) => without(m, cid));
    if (change) onChanged();
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
        ? await create(editing)
        : (await apply(commands.editComment(review.id, id, editing))) && id;
    if (cid === undefined) return undefined;
    setEdits((m) => without(m, cid));
    return cid;
  }

  /** Adds the block's text to the PR's pending review, saving it first. */
  async function publish(id: number) {
    if (!review || publishing.has(id)) return;
    const cid = await saveEdit(id);
    if (cid === undefined) return;
    setPublishing((s) => new Set(s).add(cid));
    await apply(commands.publishComment(review.id, cid));
    setPublishing((s) => {
      const next = new Set(s);
      next.delete(cid);
      return next;
    });
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
    stop: commands.stopWorkshop,
    use,
    send: sendComment,
    publish,
    publishing,
    canPublish,
    /** The agent edits `@`, so a change only shows when the review includes it. */
    canChange: review?.hasWorkingCopy ?? false,
    done: onDone,
  };
}

export type Comments = ReturnType<typeof useComments>;
