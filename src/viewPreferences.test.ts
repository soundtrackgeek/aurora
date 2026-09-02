import { describe, expect, it } from "vitest";
import {
  defaultViewPreferences,
  explorerViewForDestination,
  loadViewPreferences,
  saveViewPreferences,
  shouldRetargetTagsForAlbumSelection,
  shouldUseExplorerTagSelection,
  type ViewPreferences,
} from "./viewPreferences";

describe("destination explorer routing", () => {
  it("changes explorer views only for destinations that actually use the explorer", () => {
    expect(explorerViewForDestination("Universe")).toBe("tracks");
    expect(explorerViewForDestination("Songs")).toBe("tracks");
    expect(explorerViewForDestination("Tags")).toBe("tracks");
    expect(explorerViewForDestination("Albums")).toBe("albums");
    expect(explorerViewForDestination("Artists")).toBe("artists");
    expect(explorerViewForDestination("Inbox")).toBeNull();
    expect(explorerViewForDestination("Charts")).toBeNull();
    expect(explorerViewForDestination("History")).toBeNull();
  });
});

describe("tag selection scope", () => {
  it("does not let a hidden Albums selection override the Ratings inspector", () => {
    expect(shouldUseExplorerTagSelection("Albums")).toBe(true);
    expect(shouldUseExplorerTagSelection("Ratings")).toBe(false);
  });

  it("preserves an explicit Tags target while browsing completion albums", () => {
    expect(shouldRetargetTagsForAlbumSelection("tags")).toBe(false);
    expect(shouldRetargetTagsForAlbumSelection("track")).toBe(true);
    expect(shouldRetargetTagsForAlbumSelection("album")).toBe(true);
  });
});

function memoryStorage(initial: string | null = null) {
  let value = initial;
  return {
    getItem: () => value,
    setItem: (_key: string, next: string) => {
      value = next;
    },
    value: () => value,
  };
}

describe("view preferences", () => {
  it("round-trips the destination, exact explorer controls, and inspector tab", () => {
    const storage = memoryStorage();
    const preferences: ViewPreferences = {
      activeNav: "Albums",
      explorerView: "albums",
      explorerFilters: {
        query: "year:2000 NOT genre:scores OR soundtrack",
        rating: "all",
        love: "all",
        yearFrom: null,
        yearTo: null,
        yearBasis: "original",
        yearMissing: false,
        genre: null,
        artist: null,
        sort: "artistAsc",
      },
      inspectorView: "tags",
      tagSelectionKind: "album",
      selectedAlbumId: "album-2000",
    };

    expect(saveViewPreferences(preferences, storage)).toBe(true);
    expect(loadViewPreferences(storage)).toEqual(preferences);
    expect(JSON.parse(storage.value() ?? "{}").schemaVersion).toBe(1);
  });

  it("defaults safely when storage is missing, malformed, or internally inconsistent", () => {
    expect(loadViewPreferences(memoryStorage())).toEqual(defaultViewPreferences);
    expect(loadViewPreferences(memoryStorage("not json"))).toEqual(defaultViewPreferences);
    expect(loadViewPreferences(memoryStorage(JSON.stringify({
      schemaVersion: 1,
      activeNav: "Albums",
      explorerView: "albums",
      explorerFilters: {
        query: "year:2000",
        rating: "all",
        love: "all",
        yearFrom: null,
        yearTo: null,
        yearBasis: "original",
        yearMissing: false,
        genre: null,
        artist: null,
        sort: "albumAsc",
      },
      inspectorView: "tags",
      tagSelectionKind: "album",
      selectedAlbumId: null,
    })))).toEqual(defaultViewPreferences);
  });
});
