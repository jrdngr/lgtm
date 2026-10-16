import type { AssessmentDto, RiskLevel } from "../../bindings";
import { AgentButton } from "../../ui/AgentButton";
import { Icon } from "../../ui/icons";
import type { WorkspaceName } from "../../workspaces/context";
import { type ArchRun, runAssess, stopAssess } from "../runs";
import { Meter } from "./parts";

const RISK: Record<RiskLevel, [string, string]> = {
  minimal: ["Minimal risk", "green"],
  low: ["Low risk", "green"],
  moderate: ["Moderate risk", "amber"],
  high: ["High risk", "red"],
  critical: ["Critical risk", "red"],
};

interface Props {
  workspace: WorkspaceName;
  review: number;
  revset: string;
  assessment: AssessmentDto | null | undefined;
  run: ArchRun | undefined;
}

/** The score from Assess with its risk and why, and Assess or Stop. */
export function Score({ workspace, review, revset, assessment, run }: Props) {
  const running = !!run?.running;
  const error = !running && run?.error;
  const [risk, tone] = assessment ? RISK[assessment.risk] : [];
  return (
    <div className="arch-score">
      <Meter score={assessment?.score ?? 0} />
      <span className="arch-score-mid">
        {running ? (
          <span className="arch-busy">
            <span className="note-spin" />
            <span className="arch-busy-text">{run?.note}</span>
          </span>
        ) : (
          risk && <span className={`pill ${tone}`}>{risk}</span>
        )}
      </span>
      {running ? (
        <button type="button" onClick={() => stopAssess(review)}>
          <Icon name="stop" size={12} />
          Stop
        </button>
      ) : (
        <AgentButton onClick={() => runAssess(workspace, review, revset)}>Assess</AgentButton>
      )}
      {error && <span className="arch-why error">{error}</span>}
      {assessment && <span className="arch-why">{assessment.reason}</span>}
    </div>
  );
}
