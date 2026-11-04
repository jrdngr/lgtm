import { useEffect, useRef, useState } from "react";
import type { AgentCommentDto, ReviewDto, StateDto } from "../bindings";
import { type CommandResult, commands } from "../ipc";
import { agentCommentText, agentId, NEW_AGENT_COMMENT } from "./agentComments";
import { type Draft, followDraft } from "./comments";
import type { Moves } from "./follow";
import type { FlatFile } from "./model";

const NONE: AgentCommentDto[] = [];

interface Deps {
  review: ReviewDto | undefined;
  files: FlatFile[];
  moves: Moves | undefined;
  onState(state: StateDto, review?: number): void;
  onError(message: string): void;
  send(text: string, onSent?: () => void): void;
  /** Called when the user is done in a comment block, to give keys back to the review. */
  onDone(): void;
}

/** Comments for the agent: the one being written, bodies being edited, and changes to them. */
export function useAgentComments({ review, files, moves, onState, onError, send, onDone }: Deps) {
  const [draft, setDraft] = useState<Draft>();
  /** Bodies being edited, by block id. */
  const [edits, setEdits] = useState<ReadonlyMap<number, string>>(new Map());
  const [focus, setFocus] = useState<number>();
  /** Comments folded or unfolded from how they start: resolved ones folded. */
  const [toggled, setToggled] = useState<ReadonlySet<number>>(new Set());
  const comments = review?.state.agentComments ?? NONE;

  const edit = (id: number, body?: string) =>
    setEdits((m) => {
      const next = new Map(m);
      if (body === undefined) next.delete(id);
      else next.set(id, body);
      return next;
    });

  // The draft follows its rows when the diff changes, and goes when they do.
  const seen = useRef(files);
  // biome-ignore lint/correctness/useExhaustiveDependencies: only when the diff changes
  useEffect(() => {
    if (seen.current === files) return;
    seen.current = files;
    if (!draft) return;
    const next = followDraft(draft, moves);
    setDraft(next);
    if (!next) edit(NEW_AGENT_COMMENT);
  }, [files, moves]);

  async function apply(result: Promise<CommandResult<StateDto>>) {
    const id = review?.id;
    const r = await result;
    if (r.status === "error") {
      onError(r.error);
      return false;
    }
    onState(r.data, id);
    return true;
  }

  function start(next: Draft) {
    setDraft(next);
    edit(NEW_AGENT_COMMENT, edits.get(NEW_AGENT_COMMENT) ?? "");
    setFocus(NEW_AGENT_COMMENT);
  }

  async function save(id: number) {
    if (!review) return;
    const body = (edits.get(id) ?? "").trim();
    if (!body) return cancel(id);
    let ok: boolean;
    if (id === NEW_AGENT_COMMENT) {
      if (!draft) return;
      const { file, rows } = draft;
      ok = await apply(
        commands.addAgentComment(review.id, file, rows[0], rows[rows.length - 1], body),
      );
      if (ok) setDraft(undefined);
    } else ok = await apply(commands.editAgentComment(review.id, -id, body));
    if (!ok) return;
    edit(id);
    onDone();
  }

  function cancel(id: number) {
    edit(id);
    if (id === NEW_AGENT_COMMENT) setDraft(undefined);
    onDone();
  }

  async function setDone(c: AgentCommentDto, done: boolean) {
    if (!review) return;
    if (await apply(commands.setAgentCommentDone(review.id, c.number, done))) {
      setToggled((s) => {
        const next = new Set(s);
        next.delete(agentId(c.number));
        return next;
      });
    }
  }

  async function remove(c: AgentCommentDto) {
    if (review && (await apply(commands.deleteAgentComment(review.id, c.number)))) {
      edit(agentId(c.number));
      onDone();
    }
  }

  return {
    draft,
    edits,
    focus,
    comments,
    /** The number the next comment takes. */
    next: review?.state.nextAgentComment ?? 1,
    find: (id: number) => comments.find((c) => agentId(c.number) === id),
    start,
    setBody: (id: number, text: string) => edit(id, text),
    focused: () => setFocus(undefined),
    edit(c: AgentCommentDto) {
      edit(agentId(c.number), c.body);
      setFocus(agentId(c.number));
    },
    save,
    cancel,
    setDone,
    remove,
    send: (c: AgentCommentDto) => send(agentCommentText(files, c), onDone),
    toggled,
    toggle(id: number) {
      setToggled((s) => {
        const next = new Set(s);
        if (!next.delete(id)) next.add(id);
        return next;
      });
    },
  };
}

export type AgentComments = ReturnType<typeof useAgentComments>;
