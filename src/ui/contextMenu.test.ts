import { expect, test } from "vitest";
import { swatchPixels } from "./contextMenu";

const SIZE = 36;
const pixel = (px: Uint8Array, x: number, y: number) => [
  ...px.slice((y * SIZE + x) * 4, (y * SIZE + x) * 4 + 4),
];

test("a swatch is a dot of its color on a clear square", () => {
  const px = swatchPixels("#f0883e");
  expect(pixel(px, 18, 18)).toEqual([0xf0, 0x88, 0x3e, 255]);
  expect(pixel(px, 0, 0)[3]).toBe(0);
  // Between the dot and where a ring would be.
  expect(pixel(px, 18 + 14, 18)[3]).toBe(0);
});

test("a ringed swatch has a ring clear of the dot", () => {
  const px = swatchPixels("#58a6ff", true);
  expect(pixel(px, 18 + 14, 18)[3]).toBe(255);
  expect(pixel(px, 18 + 11, 18)[3]).toBe(0);
});
