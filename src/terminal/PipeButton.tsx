import { Icon } from "../ui/icons";
import { submitNextSend } from "./registry";
import "./pipe.css";

interface Props extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** A shortcut shown after the label, like `⌘↵`. */
  keys?: string;
  /** It sends to the background terminal. */
  background?: boolean;
}

/** A button that pastes something into a terminal, with a prompt glyph: teal for the
 * interactive terminal, violet for the background one. Its label names what goes. When an
 * agent runs in the terminal, it also presses Enter. */
export function PipeButton({ keys, background, className, children, onClick, ...rest }: Props) {
  return (
    <button
      type="button"
      className={`pipe${background ? " bg" : ""}${className ? ` ${className}` : ""}`}
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
