import { describe, expect, it } from "vitest";
import {
  defaultViewPreferences,
  loadViewPreferences,
  saveViewPreferences,
  type ViewPreferences,
} from "./viewPreferences";

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
        sort: "newest",
      },
      inspectorView: "tags",
      tagSelectionKind: "album",
      selectedAlbumId: null,
    })))).toEqual(defaultViewPreferences);
  });
});
