import { describe, expect, it } from "vitest";
import { imageType } from "./image";

describe("imageType", () => {
  it("knows raster images by extension, in any case, and leaves SVG and text to the editor", () => {
    expect(imageType("docs/shots/A.PNG")).toBe("image/png");
    expect(imageType("icons/icon.jpeg")).toBe("image/jpeg");
    expect(imageType("icons/icon.svg")).toBeUndefined();
    expect(imageType("png")).toBeUndefined();
    expect(imageType("src/png.rs")).toBeUndefined();
  });
});
