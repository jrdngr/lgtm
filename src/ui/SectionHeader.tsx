import type { ReactNode } from "react";

interface Props {
  title: ReactNode;
  /** Quiet text after the title, such as a count. */
  meta?: ReactNode;
  /** Icon buttons and menus on the right. */
  children?: ReactNode;
}

/** The header atop a list in a side panel: a title, a count, and its actions. */
export function SectionHeader({ title, meta, children }: Props) {
  return (
    <div className="section-header">
      <span className="section-title">{title}</span>
      {meta !== undefined && <span className="section-meta">{meta}</span>}
      <span className="spacer" />
      {children}
    </div>
  );
}

/** What an empty list says, with the key that fills it. */
export function Empty({ children }: { children: ReactNode }) {
  return <div className="placeholder">{children}</div>;
}
