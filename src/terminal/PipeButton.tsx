import { useId } from "react";
import { Spinner } from "../ui/Busy";
import { Icon } from "../ui/icons";
import { submitNextSend } from "./registry";
import { claimSend, useSending } from "./sending";
import "./pipe.css";

interface Props extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** A shortcut shown after the label, like `⌘↵`. */
  keys?: string;
  /** It sends to the background terminal. */
  background?: boolean;
  /** The button that spins while a background send is worked on, when not this one. */
  sendKey?: string;
}

/** A button that pastes something into a terminal, with a prompt glyph: teal for the
 * interactive terminal, violet for the background one. Its label names what goes. When an
 * agent runs in the terminal, it also presses Enter. A background send's glyph spins while
 * the agent works on it. */
export function PipeButton({
  keys,
  background,
  sendKey,
  className,
  children,
  onClick,
  ...rest
}: Props) {
  const own = useId();
  const key = sendKey ?? own;
  const spinning = useSending(key) && background && !sendKey;
  return (
    <button
      type="button"
      className={`pipe${background ? " bg" : ""}${className ? ` ${className}` : ""}`}
      onClick={(e) => {
        submitNextSend();
        if (background) claimSend(key);
        onClick?.(e);
      }}
      {...rest}
    >
      {spinning ? <Spinner /> : <Icon name="terminal" size={13} />}
      {children}
      {keys && <kbd>{keys}</kbd>}
    </button>
  );
}
