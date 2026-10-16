import { Icon } from "./icons";
import "./agent.css";

interface Props extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** A shortcut shown after the label, like `t`. */
  keys?: string;
}

/** A button that runs a headless agent: Claude orange, with a sparkle. */
export function AgentButton({ keys, className, children, ...rest }: Props) {
  return (
    <button type="button" className={`agent-run${className ? ` ${className}` : ""}`} {...rest}>
      <Icon name="agent" size={13} />
      {children}
      {keys && <kbd>{keys}</kbd>}
    </button>
  );
}
