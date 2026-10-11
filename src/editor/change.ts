/** The smallest single replacement turning `from` into `to`, so a reload keeps the cursor
 * and scroll position wherever the text around them didn't change. */
export function minimalChange(from: string, to: string) {
  const max = Math.min(from.length, to.length);
  let start = 0;
  while (start < max && from.charCodeAt(start) === to.charCodeAt(start)) start++;
  let end = 0;
  while (
    end < max - start &&
    from.charCodeAt(from.length - 1 - end) === to.charCodeAt(to.length - 1 - end)
  )
    end++;
  return { from: start, to: from.length - end, insert: to.slice(start, to.length - end) };
}
