import { describe, expect, it } from "vitest";
import { nextSeekBarColour, seekBarColour } from "./soundMap";

describe("seek bar colours", () => {
  it("runs from dull blue to bright gold and narrow orange to wide cyan", () => {
    expect(seekBarColour("brightness", 0)).toBe("rgb(59, 76, 192)");
    expect(seekBarColour("brightness", 1)).toBe("rgb(255, 209, 102)");
    expect(seekBarColour("brightness", 0.5)).toBe("rgb(192, 86, 214)");
    expect(seekBarColour("width", 0)).toBe("rgb(255, 138, 61)");
    expect(seekBarColour("width", 1)).toBe("rgb(34, 211, 238)");
    expect(seekBarColour("width", 2)).toBe("rgb(34, 211, 238)");
  });

  it("greys out silence", () => {
    expect(seekBarColour("width", null)).toBe("rgb(91, 101, 115)");
  });

  it("cycles brightness, width, plain", () => {
    expect(nextSeekBarColour("brightness")).toBe("width");
    expect(nextSeekBarColour("width")).toBe("plain");
    expect(nextSeekBarColour("plain")).toBe("brightness");
  });
});
