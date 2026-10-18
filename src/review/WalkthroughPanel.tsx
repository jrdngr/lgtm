import { useWorkspace } from "../core/workspace";
import { openSettings } from "../settings/open";
import { contextMenu } from "../ui/contextMenu";
import { IconButton } from "../ui/icons";
import { Menu } from "../ui/Menu";
import { Empty, SectionHeader } from "../ui/SectionHeader";
import { useCurrentReview } from "./current";
import { setTourState, useTourState } from "./tour";
import { WalkthroughTour } from "./WalkthroughPanelTour";
import "./notes.css";
import { PipeButton } from "../ui/PipeButton";

/** The current review's walkthrough: its notes along the chosen tour, filtered by category.
 * Click a note to go to it; `n` and `N` in the review step through the tour. */
export function WalkthroughPanel() {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
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
  const write = {
    label: "Walkthrough in Terminal",
    keys: "h",
    title: "h",
    onSelect: review.walkthroughInTerminal,
  };
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
        {notes.length > 0 ? (
          <WalkthroughTour review={review} />
        ) : (
          <div className="note-empty">
            <PipeButton onClick={review.walkthroughInTerminal} keys="h">
              Walkthrough
            </PipeButton>
          </div>
        )}
      </div>
    </div>
  );
}
