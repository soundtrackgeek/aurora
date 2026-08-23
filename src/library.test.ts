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
  { id: "1", trackKey: "c:/music/sigur ros/takk/saeglopur.mp3", albumId: "album-1", title: "Sæglópur", artist: "Sigur Rós", displayArtist: "Jónsi", album: "Takk...", originalYear: 1999, releaseYear: 2005, publisher: "EMI Records", rating: 5, loved: true, loveState: "loved", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 473, genre: "Post-rock", playCount: 12 },
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

  it("maps search fields to their catalog meanings and combines them with commas", () => {
    expect(filterTracks(tracks, "artist:jónsi", null)).toEqual([tracks[0]]);
    expect(filterTracks(tracks, "artist:sigur rós", null)).toEqual([]);
    expect(filterTracks(tracks, "aartist:sigur rós,genre:post rock", null)).toEqual([tracks[0]]);
    expect(filterTracks(tracks, "album:takk,year:1999,ryear:2005", null)).toEqual([tracks[0]]);
    expect(filterTracks(tracks, "publisher:emi,title:sæglópur", null)).toEqual([tracks[0]]);
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

    const fieldedTracks = await exploreTracks({ search: "aartist:daft punk,genre:house" });
    expect(fieldedTracks.items.map((track) => track.title)).toEqual(["Digital Love"]);
    const fieldedAlbums = await exploreAlbums({ search: "title:digital love" });
    expect(fieldedAlbums.items.map((album) => album.title)).toEqual(["Discovery"]);
    const fieldedArtists = await exploreArtists({ search: "title:digital love" });
    expect(fieldedArtists.items.map((artist) => artist.name)).toEqual(["Daft Punk"]);
  });

  it("loads browser-preview album details by stable album identity", async () => {
    const detail = await loadAlbumDetail("preview-drive");
    expect(detail.album.title).toBe("Drive");
    expect(detail.tracks.map((track) => track.title)).toEqual(["A Real Hero"]);
  });
});
