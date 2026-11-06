/** A conflict's start marker: seven or more `<`, then a space or the line's end. */
const START = /^<{7,}( |$)/m;
const END = /^>{7,}( |$)/m;

/** Whether `text` has a conflict to resolve: a start marker with an end marker after it. */
export function hasConflicts(text: string): boolean {
  const start = START.exec(text);
  return !!start && END.test(text.slice(start.index));
}
