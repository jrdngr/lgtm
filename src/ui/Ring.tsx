import { Icon } from "./icons";

interface Props {
  /** Share done, 0 to 1. */
  done: number;
  /** Tooltip, such as the counts behind the share. */
  title?: string;
  /** Show the percent beside the ring. */
  percent?: boolean;
  /** Who checked it off, which colors the check: the user green, an agent cyan. */
  by?: "user" | "agent";
}

/** How much of something is reviewed: a small filling ring, or a green check once it's all
 * done. */
export function Ring({ done, title, percent, by }: Props) {
  if (done >= 1) {
    return (
      <span className={`ring-wrap done${by ? ` by-${by}` : ""}`} title={title}>
        <Icon name="check" size={13} />
      </span>
    );
  }
  const pct = Math.floor(done * 100);
  return (
    <span className="ring-wrap" title={title}>
      {percent && <span className="ring-pct">{pct}%</span>}
      <span className="ring" style={{ "--done": `${pct}%` } as React.CSSProperties} />
    </span>
  );
}
