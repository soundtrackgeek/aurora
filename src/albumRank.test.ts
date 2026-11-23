import { describe, expect, it } from "vitest";
import { albumRankPool, withAlbumYearRanks, type AlbumSummary } from "./library";

function album(id: string, originalYear: number | null, genre: string, albumScore: number | null, rating = 3): AlbumSummary {
  return {
    id, title: id, artist: "Artist", releaseYear: 1990, originalYear, genre, albumScore, rating,
    totalTracks: 10, ratedTracks: 10, lovedTracks: 0, durationSeconds: 3_000,
  };
}

describe("album year ranks", () => {
  it("ranks by Album Score alone, within the original year", () => {
    const ranks = Object.fromEntries(withAlbumYearRanks([
      // A higher star rating never outranks a higher Album Score.
      album("high-rating", 2026, "Rock", 8, 5),
      album("high-score", 2026, "Rock", 9.5, 2),
      album("tie", 2026, "Synthpop", 9.5, 1),
      album("other-year", 2025, "Rock", 1),
      album("unscored", 2026, "Rock", null),
      album("no-year", null, "Rock", 50),
    ]).map((entry) => [entry.id, entry.yearRank]));
    expect(ranks).toEqual({
      "high-score": 1, tie: 1, "high-rating": 3, "other-year": 1, unscored: null, "no-year": null,
    });
  });

  it("ranks every album among the fully rated ones only", () => {
    const partial = { ...album("partial", 2026, "Rock", 400), ratedTracks: 1 };
    const ranks = Object.fromEntries(withAlbumYearRanks([
      partial,
      album("done-high", 2026, "Rock", 9),
      album("done-low", 2026, "Rock", 3),
      { ...album("partial-mid", 2026, "Rock", 5), ratedTracks: 4 },
    ]).map((entry) => [entry.id, entry.yearRank]));
    // A partly rated album never pushes a finished one down.
    expect(ranks).toEqual({ partial: 1, "done-high": 1, "done-low": 2, "partial-mid": 2 });
  });

  it("keeps scores and soundtracks in their own pools", () => {
    const ranks = Object.fromEntries(withAlbumYearRanks([
      album("pop", 2026, "Pop", 4),
      album("tv", 2026, "TV", 600),
      album("game", 2026, "Video Game", 5),
      album("ost", 2026, "Soundtrack", 300),
    ]).map((entry) => [entry.id, entry.yearRank]));
    expect(ranks).toEqual({ pop: 1, tv: 1, game: 2, ost: 1 });
    expect(albumRankPool(" drama ")).toBe("scores");
    expect(albumRankPool("Soundtrack")).toBe("soundtrack");
    expect(albumRankPool(null)).toBe("general");
  });
});
