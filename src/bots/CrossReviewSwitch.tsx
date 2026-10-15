/** The run form's Cross-review switch. */
export function CrossReviewSwitch({ on, onChange }: { on: boolean; onChange(on: boolean): void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      className="bots-switch bots-cross"
      onClick={() => onChange(!on)}
    >
      <span className="bots-switch-track" />
      Cross-review
    </button>
  );
}
