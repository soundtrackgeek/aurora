import { describe, expect, it } from "vitest";
import { chartAlbumSearchQuery, chartPresets, loadChartPage, type ChartPageRequest } from "./charts";
import { filterTracks, type Track } from "./library";

it("full-chart preview returns every filtered match while retaining ranks", async () => {
  const request: ChartPageRequest = { kind: "singles", source: "officialUk", scope: "period", period: chartPresets[0], selectedYear: 1985, selectedWeek: 23, yearBasis: "year", limit: 1 };
  const limited = await loadChartPage(request);
  const full = await loadChartPage({ ...request, limit: 0, libraryStatus: "inLibrary" });
  expect(limited.entries).toHaveLength(1);
  expect(full.entries.length).toBeGreaterThan(1);
  expect(full.entries).toHaveLength(full.totalEntries);
  expect(full.entries[0]).toEqual(limited.entries[0]);
  const titleSearch = await loadChartPage({ ...request, search: " KaYlEiGh ", libraryStatus: "inLibrary" });
  expect(titleSearch.entries).toHaveLength(1);
  expect(titleSearch.entries[0].title).toBe("Kayleigh");
  expect(titleSearch.entries[0].position).toBe(2);
  const artistSearch = await loadChartPage({ ...request, search: "marillion", limit: 0 });
  expect(artistSearch.entries).toEqual(titleSearch.entries);
  const empty = await loadChartPage({ ...request, limit: 0, libraryStatus: "notInLibrary" });
  expect(empty.entries).toHaveLength(empty.totalEntries);
});

function track(id: string, album: string, artist = "Huey Lewis and the News"): Track {
  return {
    id, trackKey: id, albumId: id, title: "A song", album, artist,
    releaseYear: 1983, rating: null, loved: false, loveState: "neutral",
    tagSyncState: null, canUndoTagEdit: false, durationSeconds: null, genre: null, playCount: null,
  };
}

describe("chart album library search", () => {
  it("finds every exact Sports album without artist or longer-title matches", () => {
    const tracks = [
      track("sports", "Sports"),
      track("other-sports", "Sports", "Another artist"),
      track("artist", "People Can't Stop Chillin", "Sports"),
      track("longer", "Sports Illustrated"),
    ];
    const query = chartAlbumSearchQuery({ title: "Sports", matchedAlbumTitle: "Sports" });
    expect(query).toBe('album:"Sports"');
    expect(filterTracks(tracks, query, null).map((item) => item.id)).toEqual(["sports", "other-sports"]);
  });

  it("uses the matched catalog title when the chart spelling differs", () => {
    const query = chartAlbumSearchQuery({ title: "Sports", matchedAlbumTitle: "Sports (Expanded Edition)" });
    expect(filterTracks([track("expanded", "Sports (Expanded Edition)"), track("original", "Sports")], query, null).map((item) => item.id)).toEqual(["expanded"]);
  });

  it.each([null, "", "   "])("falls back to the exact chart title when the catalog title is %j", (matchedAlbumTitle) => {
    expect(chartAlbumSearchQuery({ title: "Sports", matchedAlbumTitle })).toBe('album:"Sports"');
  });

  it("treats embedded quotes and query operators as literal album text", () => {
    const title = 'The "Sports", AND OR NOT Collection';
    const query = chartAlbumSearchQuery({ title: "Sports", matchedAlbumTitle: title });
    expect(filterTracks([track("quoted", title), track("other", "Sports")], query, null).map((item) => item.id)).toEqual(["quoted"]);
  });
});
