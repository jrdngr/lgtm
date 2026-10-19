import type { AssessmentDto, RiskLevel } from "../../bindings";
import { Meter } from "./parts";

const RISK: Record<RiskLevel, [string, string]> = {
  minimal: ["Minimal risk", "green"],
  low: ["Low risk", "green"],
  moderate: ["Moderate risk", "amber"],
  high: ["High risk", "red"],
  critical: ["Critical risk", "red"],
};

/** The score from the last assessment, with its risk and why, and whether the revset has moved
 * since. */
export function Score({ assessment }: { assessment: AssessmentDto | null | undefined }) {
  const [risk, tone] = assessment ? RISK[assessment.risk] : [];
  return (
    <div className="arch-score">
      <Meter score={assessment?.score ?? 0} />
      {assessment && (
        <span className="arch-reason">
          <span className={`arch-risk ${tone}`}>{risk}</span>
          {assessment.stale && <span className="arch-risk">Stale</span>}
          {assessment.reason}
        </span>
      )}
    </div>
  );
}
