import type { Take } from "../bindings";
import { VERDICTS } from "./stages";

/** The agent's verdict on a thread, as a colored pill. */
export function TakePill({ take }: { take: Take | null }) {
  if (!take) return null;
  const [label, tone] = VERDICTS[take.verdict];
  return <span className={`pill ${tone}`}>{label}</span>;
}
