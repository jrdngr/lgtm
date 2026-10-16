import { IconButton } from "../ui/icons";
import { isMac } from "../workbench/keys";
import { stepZoom } from "./htmlNav";

/** The zoom action for ⌥⌘= / ⌥⌘- / ⌥⌘0 / ⌥⌘9, by key code since ⌥ changes the character. */
export function zoomKey(e: React.KeyboardEvent): "in" | "out" | "actual" | "fit" | undefined {
  const mod = isMac ? e.metaKey : e.ctrlKey;
  if (!mod || !e.altKey) return undefined;
  const keys = { Equal: "in", Minus: "out", Digit0: "actual", Digit9: "fit" } as const;
  return keys[e.code as keyof typeof keys];
}

interface Props {
  zoom: number;
  setZoom(zoom: number): void;
  fit(): void;
  fitLabel: string;
}

/** Zoom out, the zoom as a percentage, zoom in, actual size and fit. */
export function ZoomControls({ zoom, setZoom, fit, fitLabel }: Props) {
  return (
    <>
      <IconButton icon="minus" label="Zoom Out (⌥⌘-)" onClick={() => setZoom(stepZoom(zoom, -1))} />
      <span className="zoom-pct">{Math.round(zoom * 100)}%</span>
      <IconButton icon="plus" label="Zoom In (⌥⌘=)" onClick={() => setZoom(stepZoom(zoom, 1))} />
      <button
        type="button"
        className="icon-button zoom-actual"
        title="Actual Size (⌥⌘0)"
        aria-label="Actual Size (⌥⌘0)"
        onClick={() => setZoom(1)}
      >
        1:1
      </button>
      <IconButton icon="fit" label={`${fitLabel} (⌥⌘9)`} onClick={fit} />
    </>
  );
}
