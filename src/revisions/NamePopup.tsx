import { useState } from "react";
import type { Motion } from "./motions";

/** A name for `motion` to use, typed in place of the motions. */
export function NamePopup({
  motion,
  onName,
  onCancel,
}: {
  motion: Motion;
  onName: (name: string) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");

  function onKeyDown(e: React.KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      onCancel();
    } else if (e.key === "Enter" && !e.nativeEvent.isComposing) {
      e.preventDefault();
      if (name.trim()) onName(name.trim());
    }
  }

  return (
    <div className="mo-pop">
      <div className="mo-head">
        <kbd>⌘J</kbd>
        <span className="mo-typed">{motion.keys}…</span>
        <span className="spacer" />
        <kbd>esc</kbd>
      </div>
      <div className="mo">
        <span>{motion.label}</span>
        <input
          className="mo-name"
          // biome-ignore lint/a11y/noAutofocus: the popup opens to take the name
          autoFocus
          aria-label={motion.label}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={onKeyDown}
          onBlur={onCancel}
        />
      </div>
    </div>
  );
}
