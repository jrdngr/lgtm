import { PipeButton } from "../terminal/PipeButton";

interface Props {
  /** The open notes not yet sent. */
  count: number;
  onSend(): void;
  className?: string;
}

/** Sends the review's unsent notes to the agent's terminal at once. */
export function SendNotesButton({ count, onSend, className }: Props) {
  return (
    <PipeButton className={className} disabled={count === 0} onClick={onSend}>
      Send{count > 0 ? ` ${count}` : ""}
    </PipeButton>
  );
}
