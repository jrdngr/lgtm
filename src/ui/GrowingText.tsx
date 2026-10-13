import { type TextareaHTMLAttributes, useLayoutEffect, useRef } from "react";

/** A one-line textarea that wraps and grows with its text up to `maxLines`, then scrolls. */
export function GrowingText({
  maxLines = 5,
  value,
  style,
  ...rest
}: TextareaHTMLAttributes<HTMLTextAreaElement> & { maxLines?: number; value: string }) {
  const ref = useRef<HTMLTextAreaElement>(null);

  // biome-ignore lint/correctness/useExhaustiveDependencies: re-measure when the text changes
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const cs = getComputedStyle(el);
    const line = Number.parseFloat(cs.lineHeight) || Number.parseFloat(cs.fontSize) * 1.2;
    const chrome =
      Number.parseFloat(cs.paddingTop) +
      Number.parseFloat(cs.paddingBottom) +
      Number.parseFloat(cs.borderTopWidth) +
      Number.parseFloat(cs.borderBottomWidth);
    const max = line * maxLines + chrome;
    el.style.height = "auto";
    const want =
      el.scrollHeight +
      Number.parseFloat(cs.borderTopWidth) +
      Number.parseFloat(cs.borderBottomWidth);
    el.style.height = `${Math.min(want, max)}px`;
    el.style.overflowY = want > max ? "auto" : "hidden";
  }, [value, maxLines]);

  return (
    <textarea ref={ref} rows={1} value={value} style={{ resize: "none", ...style }} {...rest} />
  );
}
