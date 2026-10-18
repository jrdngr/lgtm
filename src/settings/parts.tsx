import type { ReactNode } from "react";

/** A titled block of settings, which the tab can scroll to by `id`. */
export function Section({
  id,
  title,
  children,
}: {
  id: string;
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="settings-section" data-section={id} aria-labelledby={`settings-${id}`}>
      <h2 id={`settings-${id}`}>{title}</h2>
      {children}
    </section>
  );
}

/** One setting: its label and help on the left, its control on the right. */
export function Row({
  label,
  help,
  htmlFor,
  children,
}: {
  label: string;
  help?: ReactNode;
  htmlFor?: string;
  children: ReactNode;
}) {
  return (
    <div className="settings-row">
      <div className="settings-label">
        <label htmlFor={htmlFor}>{label}</label>
        {help && <div className="settings-help">{help}</div>}
      </div>
      <div className="settings-control">{children}</div>
    </div>
  );
}
