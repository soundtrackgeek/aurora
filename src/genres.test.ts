import { beforeEach, describe, expect, it } from "vitest";
import {
  loadGenreDetail,
  loadGenreIndex,
  loadGenreQueue,
  loadGenreRadioSession,
  saveGenreRadioSession,
  sortGenres,
  type GenreSummary,
} from "./genres";

describe("genre atlas browser contract", () => {
  beforeEach(() => window.localStorage.clear());

  it("builds bounded genre summaries and details from preview data", async () => {
    const index = await loadGenreIndex();
    expect(index.length).toBeGreaterThan(0);
    const synthwave = index.find((genre) => genre.name === "Synthwave");
    expect(synthwave?.trackCount).toBe(1);
    const detail = await loadGenreDetail("Synthwave");
    expect(detail.summary.name).toBe("Synthwave");
    expect(detail.albums[0].title).toBe("OutRun");
    expect(detail.albums[0].year).toBe(2013);
    expect(detail.highlights[0].title).toBe("Nightcall");
  });

  it("keeps queue modes inside the selected genre and honors exclusions", async () => {
    const first = await loadGenreQueue({ genre: "Alternative", mode: "radio", limit: 100, excludeTrackKeys: [] });
    expect(first).toHaveLength(2);
    expect(first.every((track) => track.genre === "Alternative")).toBe(true);
    const refill = await loadGenreQueue({
      genre: "Alternative",
      mode: "shuffle",
      limit: 100,
      excludeTrackKeys: [first[0].trackKey],
    });
    expect(refill.map((track) => track.trackKey)).not.toContain(first[0].trackKey);
  });

  it("persists only a valid device-local radio source", () => {
    saveGenreRadioSession({ version: 1, genre: "Synthwave", mode: "rediscover" });
    expect(loadGenreRadioSession()).toEqual({ version: 1, genre: "Synthwave", mode: "rediscover" });
    window.localStorage.setItem("aurora.genre-radio.v1", JSON.stringify({ version: 1, genre: "Synthwave", mode: "invalid" }));
    expect(loadGenreRadioSession()).toBeNull();
  });

  it("sorts unexplored genres by personal plays before catalog coverage", () => {
    const base = (name: string, plays: number, ratedTracks: number): GenreSummary => ({
      name,
      plays,
      ratedTracks,
      trackCount: 100,
      albumCount: 10,
      artistCount: 5,
      lovedTracks: 0,
      durationSeconds: 1_000,
      averageRating: null,
      firstYear: null,
      lastYear: null,
      representativeAlbumId: null,
      sessions: plays,
      listenedSeconds: 0,
      lastListenedAtMs: null,
    });
    expect(sortGenres([base("Known", 8, 30), base("Unheard", 0, 0)], "unexplored")[0].name).toBe("Unheard");
  });
});
