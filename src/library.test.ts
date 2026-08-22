import { describe, expect, it } from "vitest";
import {
  exploreAlbums,
  exploreArtists,
  exploreTracks,
  filterTracks,
  formatCount,
  formatDuration,
  loadAlbumDetail,
  type Track,
} from "./library";

const tracks: Track[] = [
  { id: "1", trackKey: "c:/music/sigur ros/takk/saeglopur.mp3", albumId: "album-1", title: "Sæglópur", artist: "Sigur Rós", album: "Takk...", releaseYear: 2005, rating: 5, loved: true, loveState: "loved", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 473, genre: "Post-rock", playCount: 12 },
  { id: "2", trackKey: "c:/music/m83/hurry up/midnight city.mp3", albumId: "album-2", title: "Midnight City", artist: "M83", album: "Hurry Up, We're Dreaming", releaseYear: 2011, rating: 4.5, loved: true, loveState: "loved", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 243, genre: "Electronic", playCount: 42 },
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

  it("keeps browser-preview explorer filtering faithful to native requests", async () => {
    const trackPage = await exploreTracks({ rating: 4.5, loveState: "loved", sort: "titleAsc" });
    expect(trackPage.items.map((track) => track.title)).toEqual(["Nightcall", "On Melancholy Hill"]);
    expect(trackPage.nextCursor).toBeNull();

    const albumPage = await exploreAlbums({ artist: "M83", sort: "releaseYearDesc" });
    expect(albumPage.items).toHaveLength(1);
    expect(albumPage.items[0]).toMatchObject({ title: "Hurry Up, We're Dreaming", totalTracks: 1 });

    const artistPage = await exploreArtists({ genre: "Soundtrack", sort: "nameAsc" });
    expect(artistPage.items.map((artist) => artist.name)).toEqual(["College"]);
  });

  it("loads browser-preview album details by stable album identity", async () => {
    const detail = await loadAlbumDetail("preview-drive");
    expect(detail.album.title).toBe("Drive");
    expect(detail.tracks.map((track) => track.title)).toEqual(["A Real Hero"]);
  });
});
