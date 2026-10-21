import { IconButton } from "./icons";

/** An error under a panel's header, with × to dismiss it. */
export function ErrorLine({ text, onClose }: { text: string; onClose(): void }) {
  return (
    <div className="review-error error-line">
      <span>{text}</span>
      <IconButton icon="close" label="Dismiss" onClick={onClose} />
    </div>
  );
}
