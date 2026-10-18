import { useState } from "react";
import type { BotRun } from "../bindings";
import { useWorkspace } from "../core/workspace";
import { useCurrentReview } from "../review/current";
import { SectionHeader } from "../ui/SectionHeader";
import { ReportPrompt } from "./ReportPrompt";
import { RunList } from "./RunList";
import "./bots.css";

/** The workspace's reports. */
export function BotsPanel({ onOpen }: { onOpen(run: BotRun): void }) {
  const workspace = useWorkspace();
  const review = useCurrentReview(workspace);
  const [error, setError] = useState<string>();
  return (
    <div className="side-split">
      <SectionHeader title="Reports" />
      <div className="side-split-body bots">
        <ReportPrompt workspace={workspace} review={review} />
        {error && <div className="bots-error">{error}</div>}
        <RunList workspace={workspace} revset={review?.revset} onOpen={onOpen} onError={setError} />
      </div>
    </div>
  );
}
