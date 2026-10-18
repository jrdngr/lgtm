import type { AssessmentDto, RiskLevel } from "../../bindings";
import type { WorkspaceName } from "../../core/workspace";
import { commands } from "../../ipc";
import { useSendText } from "../../review/useSendText";
import { PipeButton } from "../../terminal/PipeButton";
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
  assessment: AssessmentDto | null | undefined;
  onError(error: string | undefined): void;
}

/** The score from the last assessment with its risk and why, whether the revset has moved
 * since, and Assess, which sends the prompt to the send target. */
export function Score({ workspace, review, assessment, onError }: Props) {
  const { sendText, picker } = useSendText(workspace, onError, () => {});
  const [risk, tone] = assessment ? RISK[assessment.risk] : [];

  async function assess() {
    const r = await commands.assessPrompt(review);
    if (r.status === "error") return onError(r.error);
    sendText(r.data);
  }

  return (
    <div className="arch-score">
      <Meter score={assessment?.score ?? 0} />
      <span className="arch-score-mid">
        {risk && <span className={`pill ${tone}`}>{risk}</span>}
        {assessment?.stale && <span className="pill">Stale</span>}
      </span>
      <PipeButton onClick={assess}>Assess</PipeButton>
      {assessment && <span className="arch-why">{assessment.reason}</span>}
      {picker}
    </div>
  );
}
