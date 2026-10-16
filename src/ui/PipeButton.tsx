import { Icon } from "./icons";
import "./pipe.css";

interface Props extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** A shortcut shown after the label, like `⌘↵`. */
  keys?: string;
}

/** A button that pastes something into a terminal: violet, with a prompt glyph. Its label
 * names what goes. */
export function PipeButton({ keys, className, children, ...rest }: Props) {
  return (
    <button type="button" className={`pipe${className ? ` ${className}` : ""}`} {...rest}>
      <Icon name="terminal" size={13} />
      {children}
      {keys && <kbd>{keys}</kbd>}
    </button>
  );
}
