import { Icon } from "../ui/icons";

interface Props {
  rendered: boolean;
  disabled?: boolean;
  /** Switches to the other view. */
  toggle(): void;
}

/** Editor and Rendered side by side, the one showing highlighted. */
export function ViewSwitch({ rendered, disabled, toggle }: Props) {
  const views = [
    { icon: "edit", label: "Editor (⇧⌘V)", on: !rendered },
    { icon: "eye", label: "Rendered (⇧⌘V)", on: rendered },
  ] as const;
  return (
    <span className="view-switch">
      {views.map((v) => (
        <button
          key={v.icon}
          type="button"
          title={v.label}
          aria-label={v.label}
          aria-pressed={v.on}
          disabled={disabled}
          onClick={() => !v.on && toggle()}
        >
          <Icon name={v.icon} />
        </button>
      ))}
    </span>
  );
}
