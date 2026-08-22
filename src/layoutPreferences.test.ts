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
    expect(saveLayoutPreferences({ leftSidebar: "icons", rightSidebar: "collapsed" }, storage)).toBe(true);
    expect(loadLayoutPreferences(storage)).toEqual({ leftSidebar: "icons", rightSidebar: "collapsed" });
  });

  it("cycles the left rail through all three modes", () => {
    expect(nextLeftSidebarMode("expanded")).toBe("icons");
    expect(nextLeftSidebarMode("icons")).toBe("collapsed");
    expect(nextLeftSidebarMode("collapsed")).toBe("expanded");
  });
});
