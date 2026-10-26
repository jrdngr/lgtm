import { TerminalPicker } from "../review/TerminalPicker";
import { useBackgroundAsk } from "./background";
import "./background.css";

/** Asks which terminal takes background sends, over the whole window. */
export function BackgroundPicker() {
  const ask = useBackgroundAsk();
  if (!ask) return null;
  return (
    <div className="background-picker">
      <TerminalPicker
        terminals={ask.terminals}
        title="Background terminal"
        onPick={(id) => ask.answer(id)}
        onClose={() => ask.answer(undefined)}
      />
    </div>
  );
}
