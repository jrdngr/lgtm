import { Icon, IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { useCurrentReview } from "./current";
import { setTourState, useTourState } from "./tour";
import { WalkthroughTour } from "./WalkthroughPanelTour";
import { useWalkthroughPrompt } from "./WalkthroughPrompt";
import { stopWalkthrough, useWalkthroughRun } from "./walkthroughRun";
import "./notes.css";

/** The current review's walkthrough: its notes along the chosen tour, filtered by category.
 * Click a note to go to it; `n` and `N` in the review step through the tour. */
export function WalkthroughPanel() {
  const review = useCurrentReview(useWorkspace());
  const run = useWalkthroughRun(review?.id);
  const state = useTourState(review?.id);
  const prompt = useWalkthroughPrompt();
  if (!review) {
    return (
      <div className="side-split">
        <SectionHeader title="Walkthrough" />
        <Empty>No review open</Empty>
      </div>
    );
  }
  const { notes } = review;
  const running = !!run?.running;
  const stop = () => stopWalkthrough(review.id);
  const error = (!running && run?.error) || prompt.error;
  return (
    <div className="side-split">
      <SectionHeader
        title="Walkthrough"
        meta={notes.length > 0 && `${notes.length} ${notes.length === 1 ? "note" : "notes"}`}
      >
        {notes.length > 0 && (
          <IconButton
            icon={state.inline ? "eye" : "eyeOff"}
            label={state.inline ? "Hide notes in the review" : "Show notes in the review"}
            aria-pressed={state.inline}
            onClick={() => setTourState(review.id, { inline: !state.inline })}
          />
        )}
        <Menu
          label="Walkthrough actions"
          items={[
            ...prompt.items,
            running
              ? { label: "Stop walkthrough", onSelect: stop }
              : { label: "Write walkthrough", title: "h", onSelect: review.runWalkthrough },
          ]}
        />
      </SectionHeader>
      <div className="side-split-body">
        {running && (
          <div className="note-run">
            <span className="note-spin" />
            <span className="note-run-text">{run?.note}</span>
            <button type="button" className="quiet" onClick={stop}>
              <Icon name="stop" size={12} />
              Stop
            </button>
          </div>
        )}
        {error && <div className="note-run error">{error}</div>}
        {notes.length > 0 ? (
          <WalkthroughTour review={review} />
        ) : (
          !running && (
            <div className="note-empty">
              <p>
                A walkthrough has an agent guide you through the change: what needs a careful look,
                how its flows run, and what to ask.
              </p>
              <button type="button" className="primary" onClick={review.runWalkthrough}>
                Write walkthrough <kbd>h</kbd>
              </button>
            </div>
          )
        )}
      </div>
    </div>
  );
}
