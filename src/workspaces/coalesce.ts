/** Runs `fire` with a burst's calls once it goes quiet for `quiet` ms, or `most` ms after
 * the burst began, whichever is first. */
export function coalesce<T>(fire: (calls: T[]) => void, quiet: number, most: number) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let began = 0;
  let calls: T[] = [];
  return (call: T) => {
    const now = Date.now();
    if (timer === undefined) began = now;
    else clearTimeout(timer);
    calls.push(call);
    timer = setTimeout(
      () => {
        timer = undefined;
        const burst = calls;
        calls = [];
        fire(burst);
      },
      Math.max(0, Math.min(quiet, began + most - now)),
    );
  };
}
