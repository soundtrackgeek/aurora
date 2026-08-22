import { describe, expect, it } from "vitest";
import {
  defaultLayoutPreferences,
  loadLayoutPreferences,
  nextLeftSidebarMode,
  saveLayoutPreferences,
} from "./layoutPreferences";

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

describe("layout preferences", () => {
  it("defaults safely when the stored preference is missing or invalid", () => {
    expect(loadLayoutPreferences(memoryStorage())).toEqual(defaultLayoutPreferences);
    expect(loadLayoutPreferences(memoryStorage("not json"))).toEqual(defaultLayoutPreferences);
    expect(loadLayoutPreferences(memoryStorage(JSON.stringify({
      schemaVersion: 1,
      leftSidebar: "wide",
      rightSidebar: "expanded",
    })))).toEqual(defaultLayoutPreferences);
  });

  it("round-trips icon-only and collapsed rail choices", () => {
    const storage = memoryStorage();
    const preferences = {
      leftSidebar: "icons" as const,
      rightSidebar: "collapsed" as const,
      libraryExpanded: false,
      playlistsExpanded: true,
    };
    expect(saveLayoutPreferences(preferences, storage)).toBe(true);
    expect(loadLayoutPreferences(storage)).toEqual(preferences);
    expect(JSON.parse(storage.value() ?? "{}").schemaVersion).toBe(2);
  });

  it("migrates the version 1 rail choices without hiding existing navigation", () => {
    const storage = memoryStorage(JSON.stringify({
      schemaVersion: 1,
      leftSidebar: "icons",
      rightSidebar: "collapsed",
    }));
    expect(loadLayoutPreferences(storage)).toEqual({
      leftSidebar: "icons",
      rightSidebar: "collapsed",
      libraryExpanded: true,
      playlistsExpanded: true,
    });
  });

  it("cycles the left rail through all three modes", () => {
    expect(nextLeftSidebarMode("expanded")).toBe("icons");
    expect(nextLeftSidebarMode("icons")).toBe("collapsed");
    expect(nextLeftSidebarMode("collapsed")).toBe("expanded");
  });
});
