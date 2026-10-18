import type { CheckState, CommentDto, NoteDto, StateDto, ThreadDto } from "../bindings";
import type { DoneFn } from "./marks";
import type { FlatFile } from "./model";
import type { Filter, RowStateFn } from "./rowState";
import type { Progress } from "./state";
import type { SendOutcome } from "./useSendText";
import { workspaceStore } from "./workspaceStore";

/** What the last active review pane shows, for panels that follow it (files, comments). */
export interface CurrentReview {
  /** The backend's review id. */
  id: number;
  revset: string;
  /** The whole review's head commit, whichever view is open. */
  head: string;
  files: FlatFile[];
  progress: Progress[];
  rowState: RowStateFn;
  /** Whether rows with a mark count as done. */
  done: DoneFn;
  checks: CheckState[];
  filter: Filter;
  comments: CommentDto[];
  /** The walkthrough, in reading order. */
  notes: NoteDto[];
  /** In a PR workspace, every thread and draft on the PR. */
  threads: ThreadDto[];
  /** Shows a new state, after a side view changed comments or threads. With `review`, only
   * while that review is still the one shown. */
  apply(state: StateDto, review?: number): void;
  /** Sends a terminal agent the prompt to write the review's walkthrough. */
  walkthroughInTerminal(): void;
  /** Scrolls the review pane to `file` and puts the cursor there. */
  jump(file: number): void;
  /** Scrolls the review pane to a comment, or its file when its code is gone. */
  jumpToComment(id: number): void;
  /** Shows `line` of `path` in the review pane, or in the editor when the review doesn't
   * change it. */
  show(path: string, line: number | null): void;
  /** Scrolls the review pane to a row, or its file when the row is hidden. */
  jumpToRow(file: number, row: number): void;
  /** Adds a note on changed `rows` of `file`, or with `draft`, a draft for the PR on GitHub.
   * Resolves to whether it worked. */
  commentOn(
    file: number,
    rows: number[],
    body: string,
    snippet: string,
    draft?: boolean,
  ): Promise<boolean>;
  /** In a PR workspace: fetches the PR again, and pulls its threads into the review. */
  refetchPr(): void;
  /** `quiet` skips the error when the review isn't the whole PR. */
  pullPr(quiet?: boolean): void;
  /** Pastes text into terminal `preferred` when it can take it, else one of the workspace's
   * terminals, asking which when there are several. `onSent` gets the terminal. */
  send(
    text: string,
    onSent?: (terminal: string) => void,
    report?: (outcome: SendOutcome) => void,
    preferred?: string,
  ): void;
}

/** One current review per workspace, since each has its own panes. */
const store = workspaceStore<CurrentReview>();

export const publishReview = store.publish;
/** Clears the workspace's current review only if `owner` is still it. */
export const retractReview = store.retract;
/** The workspace's current review, outside React. */
export const currentReview = store.get;
export const isCurrent = store.isOwner;
export const useCurrentReview = store.use;
export const onReviewChange = store.subscribe;
