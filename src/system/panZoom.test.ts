import { describe, expect, it } from "vitest";
import { fitView, MAX_ZOOM, MIN_ZOOM, zoomAt } from "./panZoom";

describe("zoomAt", () => {
  it("keeps the world point under the pointer still", () => {
    const v = { x: 10, y: 20, k: 1 };
    const z = zoomAt(v, 2, 110, 70);
    // World point (100, 50) sat at (110, 70) and still does.
    expect(z).toEqual({ k: 2, x: -90, y: -30 });
    expect(100 * z.k + z.x).toBe(110);
  });

  it("stops at the zoom limits", () => {
    expect(zoomAt({ x: 0, y: 0, k: 1 }, 100, 0, 0).k).toBe(MAX_ZOOM);
    expect(zoomAt({ x: 0, y: 0, k: 1 }, 0.001, 0, 0).k).toBe(MIN_ZOOM);
  });
});

describe("fitView", () => {
  it("shrinks a large world to the viewport and centers it", () => {
    const v = fitView(2000, 1000, 1048, 648);
    expect(v.k).toBe(0.5);
    expect(v).toMatchObject({ x: 24, y: 74 });
  });

  it("doesn't enlarge a small world", () => {
    expect(fitView(200, 100, 1000, 600)).toEqual({ k: 1, x: 400, y: 250 });
  });
});
