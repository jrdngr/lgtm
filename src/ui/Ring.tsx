import { Icon } from "./icons";

interface Props {
  /** Share done, 0 to 1. */
  done: number;
  /** Tooltip, such as the counts behind the share. */
  title?: string;
  /** Show the percent beside the ring. */
  percent?: boolean;
}

/** How much of something is reviewed: a small filling ring, or a green check once it's all
 * done. */
export function Ring({ done, title, percent }: Props) {
  const pct = Math.round(done * 100);
  if (pct >= 100) {
    return (
      <span className="ring-wrap done" title={title}>
        <Icon name="check" size={13} />
      </span>
    );
  }
  return (
    <span className="ring-wrap" title={title}>
      {percent && <span className="ring-pct">{pct}%</span>}
      <span className="ring" style={{ "--done": `${pct}%` } as React.CSSProperties} />
    </span>
  );
}
