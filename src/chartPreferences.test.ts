import { afterEach, expect, it, vi } from "vitest";
import { loadChartPreferences, saveChartPreferences } from "./chartPreferences";
import { chartPresets, type ChartPageRequest } from "./charts";

const request: ChartPageRequest = { kind: "albums", source: "vgLista", scope: "period", period: chartPresets[0], selectedYear: 1985, selectedWeek: 23, yearBasis: "releaseYear", limit: 100, filters: { country: "NO", artistType: "Group", status: "active" } };
afterEach(() => { localStorage.clear(); vi.restoreAllMocks(); });
it("remembers the US series and exact published date independently of ISO week", () => {
  const weekly: ChartPageRequest = { ...request, kind: "singles", source: "publishedUs", scope: "week", publishedChart: "Country Singles Chart", publishedWeek: "1993-01-02", selectedYear: 1993, selectedWeek: 1, limit: 1000 };
  saveChartPreferences(weekly, { artistKey: "Artist", titleKey: "Song" });
  expect(loadChartPreferences(request).request).toEqual(weekly);
});
it("round trips filters, year basis and identity without caching catalog data", () => {
  saveChartPreferences(request, { artistKey: "artist", titleKey: "album" });
  expect(loadChartPreferences({ ...request, source: "officialUk" })).toEqual({ request, selection: { artistKey: "artist", titleKey: "album" } });
});
it.each(["broken", "null", JSON.stringify({ request: { ...request, period: null } }), JSON.stringify({ request: { ...request, selectedWeek: 999 } }), JSON.stringify({ request: { ...request, filters: { country: 123 } } }), JSON.stringify({ request: { ...request, source: "norsktoppen" } })])("rejects malformed preferences: %s", (value) => {
  localStorage.setItem("aurora:charts:v1", value);
  expect(loadChartPreferences(request)).toEqual({ request, selection: null });
});
it("tolerates unavailable storage", () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("Unavailable"); });
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("Unavailable"); });
  expect(() => saveChartPreferences(request, null)).not.toThrow();
  expect(loadChartPreferences(request)).toEqual({ request, selection: null });
});
