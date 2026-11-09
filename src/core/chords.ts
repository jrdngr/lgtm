/** Runs app shortcuts by chord, like "Mod+P", for key menus that offer them. */

let runner: ((chord: string) => boolean) | undefined;

/** The workbench sets how a chord runs. */
export function setChordRunner(run: (chord: string) => boolean) {
  runner = run;
}

/** Runs the shortcut on `chord`. Returns whether there was one. */
export const runChord = (chord: string) => runner?.(chord) ?? false;
