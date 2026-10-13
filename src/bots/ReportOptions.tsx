import type { BotOptions } from "../bindings";

type Report = "raw" | "synthesis" | "minimal";

const REPORTS: [Report, string, string][] = [
  ["raw", "Raw", "Each persona's findings as they are"],
  ["synthesis", "Synthesis", "Merge every persona's findings into one set"],
  ["minimal", "Minimal", "Only the findings that matter to what the change is for"],
];

/** Which report the options ask for. The backend adds the synthesis a minimal report needs. */
const reportOf = (o: BotOptions): Report =>
  o.minimal ? "minimal" : o.synthesize ? "synthesis" : "raw";

function Switch({
  on,
  disabled,
  title,
  onChange,
  children,
}: {
  on: boolean;
  disabled?: boolean;
  title: string;
  onChange(on: boolean): void;
  children: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      className="bots-switch"
      disabled={disabled}
      title={title}
      onClick={() => onChange(!on)}
    >
      <span className="bots-switch-track" />
      {children}
    </button>
  );
}

interface Props {
  options: BotOptions;
  /** Fewer than two personas are picked, so they can't cross-review. */
  lone: boolean;
  onChange(o: BotOptions): void;
}

/** The report to make (Raw, Synthesis or Minimal), and whether personas cross-review first. */
export function ReportOptions({ options, lone, onChange }: Props) {
  const report = reportOf(options);
  const cross = options.crossReview && !lone;
  return (
    <div className="bots-options">
      <div className="bots-segments">
        {REPORTS.map(([r, label, title]) => (
          <button
            type="button"
            key={r}
            aria-pressed={r === report}
            title={title}
            onClick={() =>
              onChange({ ...options, synthesize: r === "synthesis", minimal: r === "minimal" })
            }
          >
            {label}
          </button>
        ))}
      </div>
      <Switch
        on={cross}
        disabled={lone}
        title={
          lone
            ? "Needs two or more personas"
            : "Personas challenge each other's findings, then a synthesis rules"
        }
        onChange={(on) => onChange({ ...options, crossReview: on })}
      >
        Cross-review
      </Switch>
      <div className="bots-sub">
        <Switch
          on={cross && options.adversarial}
          disabled={!cross}
          title="Cross-review argues against every medium and higher finding"
          onChange={(on) => onChange({ ...options, adversarial: on })}
        >
          Adversarial
        </Switch>
      </div>
    </div>
  );
}
