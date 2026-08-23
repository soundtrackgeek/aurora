import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  clearPublisherLogoOverride,
  loadPublisherLogoOverrides,
  publisherLogoKey,
  publisherLogoVariant,
  publisherMonogram,
  preparePublisherLogo,
  savePublisherLogoOverride,
} from "./publisherLogos";

beforeEach(() => window.localStorage.clear());
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("publisher logo helpers", () => {
  it("creates stable, compact monograms without generic label suffixes", () => {
    expect(publisherMonogram("ECM Records")).toBe("ECM");
    expect(publisherMonogram("Blue Note Records")).toBe("BN");
    expect(publisherMonogram("Motown")).toBe("M");
    expect(publisherLogoVariant("Warp Records")).toBe(publisherLogoVariant("Warp Records"));
    expect(publisherLogoKey("  Blue   Note  ")).toBe("blue note");
  });

  it("persists and clears validated device-local overrides", () => {
    const dataUrl = "data:image/png;base64,aGVsbG8=";
    const saved = savePublisherLogoOverride({}, "Parlophone", dataUrl);
    expect(loadPublisherLogoOverrides()["parlophone"]?.dataUrl).toBe(dataUrl);
    expect(clearPublisherLogoOverride(saved, "PARLOPHONE")).toEqual({});
    expect(loadPublisherLogoOverrides()).toEqual({});
  });

  it("rejects non-raster data URLs", () => {
    expect(() => savePublisherLogoOverride({}, "Parlophone", "data:image/svg+xml;base64,PHN2Zz4="))
      .toThrow("Choose a PNG, JPEG, or WebP image.");
  });

  it("pads and resizes a valid raster into a bounded local WebP", async () => {
    const OriginalUrl = URL;
    class PreviewUrl extends OriginalUrl {
      static createObjectURL = vi.fn(() => "blob:publisher-logo");
      static revokeObjectURL = vi.fn();
    }
    class PreviewImage {
      naturalWidth = 400;
      naturalHeight = 200;
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      set src(_value: string) {
        queueMicrotask(() => this.onload?.());
      }
    }
    vi.stubGlobal("URL", PreviewUrl);
    vi.stubGlobal("Image", PreviewImage);
    const drawImage = vi.fn();
    const canvas = document.createElement("canvas");
    vi.spyOn(canvas, "getContext").mockReturnValue({
      imageSmoothingEnabled: false,
      imageSmoothingQuality: "low",
      drawImage,
    } as unknown as CanvasRenderingContext2D);
    vi.spyOn(canvas, "toDataURL").mockReturnValue("data:image/webp;base64,aGVsbG8=");
    const createElement = document.createElement.bind(document);
    vi.spyOn(document, "createElement").mockImplementation((tagName) => tagName === "canvas" ? canvas : createElement(tagName));

    await expect(preparePublisherLogo(new File(["image"], "logo.png", { type: "image/png" })))
      .resolves.toBe("data:image/webp;base64,aGVsbG8=");
    expect(canvas.width).toBe(192);
    expect(canvas.height).toBe(192);
    expect(drawImage).toHaveBeenCalledWith(expect.any(PreviewImage), 14, 55, 164, 82);
    expect(PreviewUrl.revokeObjectURL).toHaveBeenCalledWith("blob:publisher-logo");
  });
});
