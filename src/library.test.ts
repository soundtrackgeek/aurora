import { describe, expect, it } from "vitest";
import {
  applyAlbumTrackTagProjection,
  applyEditableTrackTagProjection,
  applyTrackTagProjection,
  catalogRefreshIsConsistent,
  exploreAlbums,
  exploreArtists,
  exploreTracks,
  filterTracks,
  formatCount,
  formatDuration,
  loadAlbumDetail,
  type Track,
  type AlbumSummary,
} from "./library";

const tracks: Track[] = [
  { id: "1", trackKey: "c:/music/sigur ros/takk/saeglopur.mp3", albumId: "album-1", title: "Sæglópur", artist: "Sigur Rós", displayArtist: "Jónsi", album: "Takk...", originalYear: 1999, releaseYear: 2005, publisher: "EMI Records", originCountryCode: "IS", originCountryName: "Iceland", rating: 5, loved: true, loveState: "loved", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 473, genre: "Post-rock", playCount: 12 },
  { id: "2", trackKey: "c:/music/m83/hurry up/midnight city.mp3", albumId: "album-2", title: "Midnight City", artist: "M83", album: "Hurry Up, We're Dreaming", releaseYear: 2011, originCountryCode: "FR", originCountryName: "France", rating: 4.5, loved: true, loveState: "loved", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 243, genre: "Electronic", playCount: 42 },
];

const similarlyNamedArtists: Track[] = [
  { ...tracks[0], id: "3", trackKey: "c:/music/kiss/strutter.mp3", albumId: "album-3", title: "Strutter", artist: "Kiss", displayArtist: "Kiss", album: "Kiss", originalYear: 1974, releaseYear: 1974, genre: "Rock" },
  { ...tracks[1], id: "4", trackKey: "c:/music/kissing-the-pink/certain-things.mp3", albumId: "album-4", title: "Certain Things Are Likely", artist: "Kissing the Pink", album: "Certain Things Are Likely", releaseYear: 1986, genre: "Synth-pop" },
];

const scoreTracks: Track[] = [
  { ...tracks[0], id: "5", trackKey: "c:/music/composer/main-theme.mp3", albumId: "album-5", title: "Main Theme", artist: "Composer", album: "Film Music", genre: "Drama" },
  { ...tracks[1], id: "6", trackKey: "c:/music/singer/pop-song.mp3", albumId: "album-6", title: "Pop Song", artist: "Singer", album: "Compilation", genre: "Soundtrack" },
];

const yearRangeTracks: Track[] = [
  { ...tracks[0], id: "7", trackKey: "c:/music/ranges/1985.mp3", albumId: "album-7", title: "Start", originalYear: 1985, releaseYear: 1989 },
  { ...tracks[0], id: "8", trackKey: "c:/music/ranges/1987.mp3", albumId: "album-8", title: "End", originalYear: 1987, releaseYear: 1986 },
  { ...tracks[0], id: "9", trackKey: "c:/music/ranges/1988.mp3", albumId: "album-9", title: "Outside", originalYear: 1988, releaseYear: 1987 },
];

describe("library presentation", () => {
  it("acknowledges a catalog refresh only when every read used one revision", () => {
    expect(catalogRefreshIsConsistent(
      "1:52:2026-08-24T10:01:00Z",
      "1:52:2026-08-24T10:01:00Z",
      "1:52:2026-08-24T10:01:00Z",
    )).toBe(true);
    expect(catalogRefreshIsConsistent(
      "1:52:2026-08-24T10:01:00Z",
      "1:51:2026-08-24T10:00:00Z",
      "1:52:2026-08-24T10:01:00Z",
    )).toBe(false);
    expect(catalogRefreshIsConsistent(
      "2:53:2026-08-24T10:03:00Z",
      "1:53:2026-08-24T10:02:00Z",
      "2:53:2026-08-24T10:03:00Z",
    )).toBe(false);
  });

  it("applies tag state without restoring stale catalog identity", () => {
    const current = { ...tracks[0], id: "fresh-id", albumId: "fresh-album" };
    const staleUpdate = {
      ...tracks[0],
      id: "old-id",
      albumId: "old-album",
      title: "Missing projection title",
      displayArtist: null,
      originalYear: undefined,
      publisher: undefined,
      rating: 3.5,
      loved: false,
      loveState: "banned" as const,
      tagSyncState: "pendingImport" as const,
      canUndoTagEdit: true,
    };

    expect(applyTrackTagProjection(current, staleUpdate)).toMatchObject({
      id: "fresh-id",
      albumId: "fresh-album",
      trackKey: current.trackKey,
      rating: 3.5,
      loveState: "banned",
      tagSyncState: "pendingImport",
      canUndoTagEdit: true,
    });
    expect(applyTrackTagProjection(current, staleUpdate)).toMatchObject({
      title: current.title,
      displayArtist: current.displayArtist,
      originalYear: current.originalYear,
      publisher: current.publisher,
    });
  });

  it("applies complete editable metadata only for a full-editor result", () => {
    const current = { ...tracks[0], id: "fresh-id", albumId: "fresh-album" };
    const update = {
      ...tracks[0],
      id: "old-id",
      albumId: "old-album",
      title: "Svefn-g-englar",
      artist: "Sigur Rós & Friends",
      displayArtist: null,
      album: "Ágætis byrjun",
      originalYear: null,
      publisher: null,
      trackNumber: 1,
      trackTotal: 10,
      discNumber: 1,
      discTotal: 1,
    };

    expect(applyEditableTrackTagProjection(current, update)).toMatchObject({
      id: "fresh-id",
      albumId: "fresh-album",
      title: "Svefn-g-englar",
      artist: "Sigur Rós & Friends",
      displayArtist: null,
      album: "Ágætis byrjun",
      originalYear: null,
      publisher: null,
      trackTotal: 10,
      discTotal: 1,
    });
  });

  it("projects consistent album metadata from a complete album editor result", () => {
    const album: AlbumSummary = {
      id: "album-1",
      title: "Takk...",
      artist: "Sigur Rós",
      originalYear: 1999,
      releaseYear: 2005,
      publisher: "EMI Records",
      genre: "Post-rock",
      totalTracks: 2,
      ratedTracks: 2,
      lovedTracks: 1,
      durationSeconds: 716,
      rating: 4.75,
      albumScore: 12.5,
    };
    const updatedTracks = [
      { ...tracks[0], album: "Takk", artist: "Sigur Rós & Friends", originalYear: 2000, releaseYear: 2006, publisher: "Krúnk", genre: "Art rock" },
      { ...tracks[0], id: "2", trackKey: "c:/music/sigur ros/takk/glosoli.mp3", album: "Takk", artist: "Sigur Rós & Friends", originalYear: 2000, releaseYear: 2006, publisher: "Krúnk", genre: "Art rock" },
    ];

    expect(applyAlbumTrackTagProjection(album, updatedTracks)).toEqual({
      ...album,
      title: "Takk",
      artist: "Sigur Rós & Friends",
      originalYear: 2000,
      releaseYear: 2006,
      publisher: "Krúnk",
      genre: "Art rock",
      originCountryCode: null,
      originCountryName: null,
    });
  });

  it("does not guess album metadata from a partial or mixed track result", () => {
    const album: AlbumSummary = {
      id: "album-1",
      title: "Takk...",
      artist: "Sigur Rós",
      originalYear: 1999,
      releaseYear: 2005,
      publisher: "EMI Records",
      genre: "Post-rock",
      totalTracks: 2,
      ratedTracks: 1,
      lovedTracks: 1,
      durationSeconds: 473,
      rating: null,
      albumScore: null,
    };

    expect(applyAlbumTrackTagProjection(album, [{ ...tracks[0], genre: "Art rock" }])).toBe(album);
    expect(applyAlbumTrackTagProjection(album, [
      { ...tracks[0], genre: "Art rock", publisher: null },
      { ...tracks[0], id: "2", trackKey: "second", genre: "Post-rock", publisher: null },
    ])).toEqual({
      ...album,
      publisher: null,
    });
  });

  it("formats durations and counts for dense rows", () => {
    expect(formatDuration(243)).toBe("4:03");
    expect(formatDuration(null)).toBe("—");
    expect(formatCount(12_846)).toMatch(/12.846|12,846|12 846/);
  });

  it("does not keep an old origin flag after an album-artist edit", () => {
    const album: AlbumSummary = {
      id: "album-1", title: "Takk...", artist: "Sigur Rós", originalYear: 1999,
      releaseYear: 2005, publisher: "EMI Records", originCountryCode: "IS",
      originCountryName: "Iceland", genre: "Post-rock", totalTracks: 1,
      ratedTracks: 1, lovedTracks: 1, durationSeconds: 473, rating: 5, albumScore: 95,
    };
    const projected = applyAlbumTrackTagProjection(album, [{ ...tracks[0], artist: "Jónsi" }]);
    expect(projected).toMatchObject({ artist: "Jónsi", originCountryCode: null, originCountryName: null });
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
    expect(filterTracks(tracks, "country:iceland OR france", null)).toEqual(tracks);
    expect(filterTracks(tracks, "country:is", null)).toEqual([tracks[0]]);
    expect(filterTracks(tracks, "year:2011", null)).toEqual([]);
    expect(filterTracks(tracks, "ryear:2011", null)).toEqual([tracks[1]]);
  });

  it("supports inclusive closed and open Year and Release Year ranges", () => {
    expect(filterTracks(yearRangeTracks, "year:1985..1987", null)).toEqual(yearRangeTracks.slice(0, 2));
    expect(filterTracks(yearRangeTracks, "ryear:1985..1987", null)).toEqual(yearRangeTracks.slice(1));
    expect(filterTracks(yearRangeTracks, "year:..1985", null)).toEqual([yearRangeTracks[0]]);
    expect(filterTracks(yearRangeTracks, "year:1987..", null)).toEqual(yearRangeTracks.slice(1));
    expect(filterTracks(yearRangeTracks, "year:1985..1987 OR 1988", null)).toEqual(yearRangeTracks);
    expect(filterTracks(yearRangeTracks, "NOT year:1985..1987", null)).toEqual([yearRangeTracks[2]]);
    expect(() => filterTracks(yearRangeTracks, "year:1987..1985", null)).toThrow(/start at or before/u);
    expect(() => filterTracks(yearRangeTracks, "ryear:..", null)).toThrow(/starting or ending/u);
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
    expect(trackPage.totalCount).toBe(2);

    const boundedTrackPage = await exploreTracks({ pageSize: 1, genre: "Alternative", sort: "titleAsc" });
    expect(boundedTrackPage.items).toHaveLength(1);
    expect(boundedTrackPage.totalCount).toBe(2);

    const albumPage = await exploreAlbums({ artist: "M83", sort: "releaseYearDesc" });
    expect(albumPage.items).toHaveLength(1);
    expect(albumPage.items[0]).toMatchObject({ title: "Hurry Up, We're Dreaming", totalTracks: 2 });
    expect(albumPage.totalCount).toBe(1);

    const artistPage = await exploreArtists({ genre: "Soundtrack", sort: "nameAsc" });
    expect(artistPage.items.map((artist) => artist.name)).toEqual(["College"]);
    expect(artistPage.totalCount).toBe(1);

    const fieldedTracks = await exploreTracks({ search: "aartist:daft punk,genre:house" });
    expect(fieldedTracks.items.map((track) => track.title)).toEqual(["Digital Love"]);
    const fieldedAlbums = await exploreAlbums({ search: "title:digital love" });
    expect(fieldedAlbums.items.map((album) => album.title)).toEqual(["Discovery"]);
    const fieldedArtists = await exploreArtists({ search: "title:digital love" });
    expect(fieldedArtists.items.map((artist) => artist.name)).toEqual(["Daft Punk"]);

    const booleanTracks = await exploreTracks({ search: "genre:house OR electronic NOT aartist:m83" });
    expect(booleanTracks.items.map((track) => track.title)).toEqual(["Digital Love"]);

    const yearRange = await exploreTracks({ search: "year:2008..2010", sort: "titleAsc" });
    expect(yearRange.items.map((track) => track.title)).toEqual(["Intro", "On Melancholy Hill", "Strawberry Swing"]);
    const releaseYearRange = await exploreTracks({ search: "ryear:2011..", sort: "titleAsc" });
    expect(releaseYearRange.items.map((track) => track.title)).toEqual(["A Real Hero", "Nightcall"]);
  });

  it("reverses chronological and alphabetical browser-preview sorts", async () => {
    const yearAsc = await exploreTracks({ sort: "yearAsc" });
    const yearDesc = await exploreTracks({ sort: "yearDesc" });
    expect(yearAsc.items.map((track) => track.id)).toEqual(yearDesc.items.map((track) => track.id).reverse());

    const titleAsc = await exploreTracks({ sort: "titleAsc" });
    const titleDesc = await exploreTracks({ sort: "titleDesc" });
    expect(titleAsc.items.map((track) => track.id)).toEqual(titleDesc.items.map((track) => track.id).reverse());

    const albumYearAsc = await exploreAlbums({ sort: "yearAsc" });
    const albumYearDesc = await exploreAlbums({ sort: "yearDesc" });
    expect(albumYearAsc.items.map((album) => album.id)).toEqual(albumYearDesc.items.map((album) => album.id).reverse());

    const albumAddedAsc = await exploreAlbums({ sort: "oldest" });
    const albumAddedDesc = await exploreAlbums({ sort: "newest" });
    expect(albumAddedAsc.items.map((album) => album.id)).toEqual(albumAddedDesc.items.map((album) => album.id).reverse());

    const artistAsc = await exploreArtists({ sort: "nameAsc" });
    const artistDesc = await exploreArtists({ sort: "nameDesc" });
    expect(artistAsc.items.map((artist) => artist.id)).toEqual(artistDesc.items.map((artist) => artist.id).reverse());
  });

  it("loads browser-preview album details by stable album identity", async () => {
    const detail = await loadAlbumDetail("preview-drive");
    expect(detail.album.title).toBe("Drive");
    expect(detail.tracks.map((track) => track.title)).toEqual(["A Real Hero"]);
  });
});
