/** Runs `fire` once a burst of calls goes quiet for `quiet` ms, or `most` ms after the
 * burst began, whichever is first. */
export function coalesce(fire: () => void, quiet: number, most: number): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let began = 0;
  return () => {
    const now = Date.now();
    if (timer === undefined) began = now;
    else clearTimeout(timer);
    timer = setTimeout(
      () => {
        timer = undefined;
        fire();
      },
      Math.max(0, Math.min(quiet, began + most - now)),
    );
  };
}
