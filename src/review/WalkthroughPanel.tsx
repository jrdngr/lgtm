import { openSettings } from "../settings/open";
import { contextMenu } from "../ui/contextMenu";
import { Icon, IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useWorkspace } from "../workspaces/context";
import { useCurrentReview } from "./current";
import { setTourState, useTourState } from "./tour";
import { WalkthroughTour } from "./WalkthroughPanelTour";
import { stopWalkthrough, useWalkthroughRun } from "./walkthroughRun";
import "./notes.css";

/** The current review's walkthrough: its notes along the chosen tour, filtered by category.
 * Click a note to go to it; `n` and `N` in the review step through the tour. */
export function WalkthroughPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const run = useWalkthroughRun(review?.id);
  const state = useTourState(review?.id);
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
  const error = !running && run?.error;
  const write = running
    ? { label: "Stop Walkthrough", onSelect: stop }
    : { label: "Write Walkthrough", keys: "h", title: "h", onSelect: review.runWalkthrough };
  const editPrompt = {
    label: "Edit Prompt…",
    title: "Opens the prompt files in Settings",
    onSelect: () => openSettings(workspace, "prompts"),
  };
  const toggleInline = () => setTourState(review.id, { inline: !state.inline });
  const headerMenu = contextMenu(() => [
    write,
    notes.length > 0 && {
      label: "Show Notes in Code",
      checked: state.inline,
      onSelect: toggleInline,
    },
    "-",
    editPrompt,
  ]);
  return (
    <div className="side-split">
      <div style={{ display: "contents" }} onContextMenu={headerMenu}>
        <SectionHeader
          title="Walkthrough"
          meta={notes.length > 0 && `${notes.length} ${notes.length === 1 ? "note" : "notes"}`}
        >
          {notes.length > 0 && (
            <IconButton
              icon={state.inline ? "eye" : "eyeOff"}
              label={state.inline ? "Hide notes in the review" : "Show notes in the review"}
              aria-pressed={state.inline}
              onClick={toggleInline}
            />
          )}
          <Menu label="Walkthrough actions" items={[editPrompt, write]} />
        </SectionHeader>
      </div>
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
