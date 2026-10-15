import { afterEach, expect, it, vi } from "vitest";
import { nextRandomSortSeed } from "./randomSort";
import { exploreAlbums, exploreTracks } from "./library";
import { loadViewPreferences, saveViewPreferences, defaultViewPreferences } from "./viewPreferences";
import { loadExplorerPage } from "./app/explorer/explorerQueries";

afterEach(() => { vi.restoreAllMocks(); localStorage.clear(); });

it("keeps a random view durable and delivers its seed through both query paths", async () => {
  for (const view of ["tracks", "albums"] as const) {
    const preferences = { ...defaultViewPreferences, explorerView: view, explorerFilters: { ...defaultViewPreferences.explorerFilters, sort: "random" as const, randomSeed: 42 } };
    saveViewPreferences(preferences);
    const restored = loadViewPreferences();
    expect(restored.explorerFilters).toEqual(preferences.explorerFilters);
    const first = await loadExplorerPage(view, restored.explorerFilters);
    const repeated = await loadExplorerPage(view, restored.explorerFilters);
    const reshuffled = await loadExplorerPage(view, { ...restored.explorerFilters, randomSeed: 43 });
    expect(first).toEqual(repeated);
    expect(first[view].map((item) => item.id)).not.toEqual(reshuffled[view].map((item) => item.id));
  }
});

it("randomizes the complete matching preview set and preserves filters", async () => {
  for (const explore of [exploreTracks, exploreAlbums]) {
    const normal = await explore({ pageSize: 100, genre: "Alternative" });
    const random = await explore({ pageSize: 100, genre: "Alternative", sort: "random", randomSeed: 123 });
    expect(random.items.length).toBeGreaterThan(1);
    expect(random.items.every((item) => item.genre === "Alternative")).toBe(true);
    expect(random.items.map((item) => item.id).sort()).toEqual(normal.items.map((item) => item.id).sort());
  }
});

it("guarantees a fresh seed even if the random source repeats the previous value", () => {
  vi.spyOn(crypto, "getRandomValues").mockImplementation((array) => {
    (array as Uint32Array)[0] = 0xffff_ffff;
    return array;
  });
  expect(nextRandomSortSeed(0xffff_ffff)).toBe(0);
});
