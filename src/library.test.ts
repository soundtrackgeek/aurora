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

const similarlyNamedArtists: Track[] = [
  { ...tracks[0], id: "3", trackKey: "c:/music/kiss/strutter.mp3", albumId: "album-3", title: "Strutter", artist: "Kiss", displayArtist: "Kiss", album: "Kiss", originalYear: 1974, releaseYear: 1974, genre: "Rock" },
  { ...tracks[1], id: "4", trackKey: "c:/music/kissing-the-pink/certain-things.mp3", albumId: "album-4", title: "Certain Things Are Likely", artist: "Kissing the Pink", album: "Certain Things Are Likely", releaseYear: 1986, genre: "Synth-pop" },
];

const scoreTracks: Track[] = [
  { ...tracks[0], id: "5", trackKey: "c:/music/composer/main-theme.mp3", albumId: "album-5", title: "Main Theme", artist: "Composer", album: "Film Music", genre: "Drama" },
  { ...tracks[1], id: "6", trackKey: "c:/music/singer/pop-song.mp3", albumId: "album-6", title: "Pop Song", artist: "Singer", album: "Compilation", genre: "Soundtrack" },
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

  it("supports OR inheritance, NOT, negative fields, and exact quoted values", () => {
    expect(filterTracks(tracks, "aartist:sigur rós OR m83", null)).toEqual(tracks);
    expect(filterTracks(tracks, "genre:post rock OR electronic NOT aartist:m83", null)).toEqual([tracks[0]]);
    expect(filterTracks(tracks, "-genre:post rock", null)).toEqual([tracks[1]]);
    expect(filterTracks(tracks, "genre:post rock,-aartist:sigur", null)).toEqual([]);
    expect(filterTracks(similarlyNamedArtists, "aartist:kiss", null)).toHaveLength(2);
    expect(filterTracks(similarlyNamedArtists, "aartist:\"kiss\"", null)).toEqual([similarlyNamedArtists[0]]);
  });

  it("expands the Music Library scores umbrella without changing quoted exact search", () => {
    expect(filterTracks(scoreTracks, "genre:scores", null)).toEqual([scoreTracks[0]]);
    expect(filterTracks(scoreTracks, "genre:\"scores\"", null)).toEqual([]);
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

    const booleanTracks = await exploreTracks({ search: "genre:house OR electronic NOT aartist:m83" });
    expect(booleanTracks.items.map((track) => track.title)).toEqual(["Digital Love"]);
  });

  it("loads browser-preview album details by stable album identity", async () => {
    const detail = await loadAlbumDetail("preview-drive");
    expect(detail.album.title).toBe("Drive");
    expect(detail.tracks.map((track) => track.title)).toEqual(["A Real Hero"]);
  });
});
