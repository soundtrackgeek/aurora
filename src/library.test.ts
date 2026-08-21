import { describe, expect, it } from "vitest";
import { filterTracks, formatCount, formatDuration, type Track } from "./library";

const tracks: Track[] = [
  { id: "1", albumId: "album-1", title: "Sæglópur", artist: "Sigur Rós", album: "Takk...", releaseYear: 2005, rating: 5, loved: true, durationSeconds: 473, genre: "Post-rock", playCount: 12 },
  { id: "2", albumId: "album-2", title: "Midnight City", artist: "M83", album: "Hurry Up, We're Dreaming", releaseYear: 2011, rating: 4.5, loved: true, durationSeconds: 243, genre: "Electronic", playCount: 42 },
];

describe("library presentation", () => {
  it("formats durations and counts for dense rows", () => {
    expect(formatDuration(243)).toBe("4:03");
    expect(formatDuration(null)).toBe("—");
    expect(formatCount(12_846)).toMatch(/12.846|12,846|12 846/);
  });

  it("filters with Unicode-aware user text and selected artist", () => {
    expect(filterTracks(tracks, "sægl", null)).toHaveLength(1);
    expect(filterTracks(tracks, "", "M83")).toEqual([tracks[1]]);
    expect(filterTracks(tracks, "elect", "Sigur Rós")).toEqual([]);
  });
});
