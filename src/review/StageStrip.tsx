import type { Stage } from "../bindings";
import { STAGES } from "../model/stages";

interface Props {
  stage: Stage;
  /** Shown but not pickable, while an agent sets the stage. */
  locked?: boolean;
  onPick(stage: Stage): void;
}

/** A thread's stages side by side, the one it's in filled with its color. */
export function StageStrip({ stage, locked, onPick }: Props) {
  return (
    <fieldset className="stage-strip" aria-label="Stage" disabled={locked}>
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
