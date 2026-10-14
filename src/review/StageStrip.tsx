import type { Stage } from "../bindings";
import { STAGES } from "../threads/stages";

interface Props {
  stage: Stage;
  onPick(stage: Stage): void;
}

/** A thread's stages side by side, the one it's in filled with its color. */
export function StageStrip({ stage, onPick }: Props) {
  return (
    <fieldset className="stage-strip" aria-label="Stage">
      {STAGES.map(([s, name]) => (
        <button
          key={s}
          type="button"
          data-stage={s}
          className={s === stage ? "on" : undefined}
          aria-pressed={s === stage}
          onClick={() => s !== stage && onPick(s)}
        >
          <i />
          {name}
        </button>
      ))}
    </fieldset>
  );
}
