import { describe, expect, it } from "vitest";
import {
  defaultTrackColumnLayout,
  loadTrackColumnLayout,
  moveTrackColumn,
  sanitizeTrackColumnLayout,
  saveTrackColumnLayout,
  setTrackColumnWidth,
  toggleTrackColumn,
  visibleTrackColumns,
} from "./trackColumns";

function memoryStorage() {
  const values = new Map<string, string>();
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
  };
}

describe("track column layout", () => {
  it("hides Publisher and Charts by default", () => {
    const visible = visibleTrackColumns(defaultTrackColumnLayout());
    expect(visible).not.toContain("publisher");
    expect(visible).not.toContain("charts");
    expect(visible[0]).toBe("title");
  });

  it("moves a column before another visible column or to the end", () => {
    const layout = defaultTrackColumnLayout();
    const artistFirst = moveTrackColumn(layout, "artist", 0);
    expect(visibleTrackColumns(artistFirst).slice(0, 2)).toEqual(["artist", "title"]);
    const loveLast = moveTrackColumn(layout, "title", visibleTrackColumns(layout).length);
    expect(visibleTrackColumns(loveLast).slice(-1)[0]).toBe("title");
    expect(moveTrackColumn(layout, "title", 0)).toBe(layout);
  });

  it("keeps a hidden column's position when it is shown again", () => {
    const shown = toggleTrackColumn(defaultTrackColumnLayout(), "publisher");
    expect(visibleTrackColumns(shown).indexOf("publisher")).toBe(visibleTrackColumns(shown).indexOf("genre") + 1);
  });

  it("never hides the last visible column", () => {
    let layout = defaultTrackColumnLayout();
    for (const id of visibleTrackColumns(layout)) layout = toggleTrackColumn(layout, id);
    expect(visibleTrackColumns(layout)).toHaveLength(1);
  });

  it("clamps widths and survives a save and load", () => {
    const storage = memoryStorage();
    const layout = setTrackColumnWidth(moveTrackColumn(defaultTrackColumnLayout(), "love", 0), "artist", 5);
    expect(layout.widths.artist).toBe(60);
    saveTrackColumnLayout(layout, storage);
    expect(loadTrackColumnLayout(storage)).toEqual(layout);
  });

  it("repairs unknown, duplicate and missing columns from storage", () => {
    const layout = sanitizeTrackColumnLayout({ order: ["love", "bogus", "love", "title"], hidden: ["nope"], widths: { title: "wide", album: 9999 } });
    expect(layout.order.slice(0, 2)).toEqual(["love", "title"]);
    expect(new Set(layout.order).size).toBe(defaultTrackColumnLayout().order.length);
    expect(layout.hidden).toEqual([]);
    expect(layout.widths).toEqual({ album: 900 });
    expect(loadTrackColumnLayout({ getItem: () => "{not json", setItem: () => undefined })).toEqual(defaultTrackColumnLayout());
  });
});
