interface Props<T extends string> {
  options: [T, string][];
  value: T;
  onChange(next: T): void;
}

/** A row of buttons picking one of a side tab's views. */
export function Segmented<T extends string>({ options, value, onChange }: Props<T>) {
  return (
    <div className="segmented" role="tablist">
      {options.map(([id, label]) => (
        <button
          type="button"
          role="tab"
          key={id}
          aria-selected={id === value}
          className={id === value ? "selected" : undefined}
          onClick={() => onChange(id)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}
