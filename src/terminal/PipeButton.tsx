import { Icon } from "../ui/icons";
import { submitNextSend } from "./registry";
import "./pipe.css";

interface Props extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** A shortcut shown after the label, like `⌘↵`. */
  keys?: string;
}

/** A button that pastes something into a terminal: violet, with a prompt glyph. Its label
 * names what goes. When an agent runs in the terminal, it also presses Enter. */
export function PipeButton({ keys, className, children, onClick, ...rest }: Props) {
  return (
    <button
      type="button"
      className={`pipe${className ? ` ${className}` : ""}`}
      onClick={(e) => {
        submitNextSend();
        onClick?.(e);
      }}
      {...rest}
    >
      <Icon name="terminal" size={13} />
      {children}
      {keys && <kbd>{keys}</kbd>}
    </button>
  );
}
