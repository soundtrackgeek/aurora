import { describe, expect, it } from "vitest";
import {
  createDefaultDisplayPreferences,
  effectiveDisplayPreferences,
  loadDisplayPreferences,
  saveDisplayPreferences,
} from "./displayPreferences";

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

describe("display preferences", () => {
  it("starts with a readable global default and a stronger Charts default", () => {
    const preferences = loadDisplayPreferences(memoryStorage());

    expect(preferences.global).toEqual({ textSize: "comfortable", coverSize: "standard" });
    expect(effectiveDisplayPreferences(preferences, "songs")).toEqual({ textSize: "comfortable", coverSize: "standard" });
    expect(effectiveDisplayPreferences(preferences, "charts")).toEqual({ textSize: "large", coverSize: "standard" });
  });

  it("round-trips global choices and independent view overrides", () => {
    const storage = memoryStorage();
    const preferences = createDefaultDisplayPreferences();
    preferences.global = { textSize: "large", coverSize: "large" };
    preferences.views.albums = { textSize: "maximum", coverSize: "extra-large" };

    expect(saveDisplayPreferences(preferences, storage)).toBe(true);
    expect(loadDisplayPreferences(storage)).toEqual(preferences);
    expect(JSON.parse(storage.value() ?? "{}").schemaVersion).toBe(1);
  });

  it("rejects an invalid schema and repairs invalid individual overrides", () => {
    expect(loadDisplayPreferences(memoryStorage(JSON.stringify({ schemaVersion: 2 })))).toEqual(createDefaultDisplayPreferences());

    const stored = createDefaultDisplayPreferences();
    const storage = memoryStorage(JSON.stringify({
      schemaVersion: 1,
      ...stored,
      views: {
        ...stored.views,
        charts: { textSize: "microscopic", coverSize: "large" },
      },
    }));
    const repaired = loadDisplayPreferences(storage);
    expect(repaired.views.charts).toEqual({ textSize: "large", coverSize: "large" });
  });
});
