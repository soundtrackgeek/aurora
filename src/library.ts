import { invoke } from "@tauri-apps/api/core";
import type { CatalogSync } from "./tags";

export type SourceState = "connected" | "unavailable" | "browser-preview";

export interface LibrarySummary {
  songs: number;
  albums: number;
  artists: number;
  genres: number;
  loved: number;
  rated: number;
}

export interface Artist {
  id: string;
  name: string;
  trackCount: number;
  albumCount: number;
  playCount: number | null;
  lastPlayedAtMs: number | null;
}

export interface Track {
  id: string;
  trackKey: string;
  albumId: string | null;
  title: string;
  artist: string;
  displayArtist?: string | null;
  album: string;
  releaseYear: number | null;
  originalYear?: number | null;
  publisher?: string | null;
  originCountryCode?: string | null;
  originCountryName?: string | null;
  rating: number | null;
  loved: boolean;
  loveState: "neutral" | "loved" | "banned";
  tagSyncState: "pendingImport" | null;
  canUndoTagEdit: boolean;
  durationSeconds: number | null;
  genre: string | null;
  playCount: number | null;
  trackNumber?: number | null;
  trackTotal?: number | null;
  discNumber?: number | null;
  discTotal?: number | null;
  lastFmAlbumRank?: number | null;
}

export function displayTrackArtist(track: Track): string {
  return track.displayArtist?.trim() || track.artist;
}

export function artistPortraitUrl(artist: string, size: 64 | 128 = 64): string | null {
  if (!isTauriRuntime() || !artist.trim()) return null;
  return `http://aurora-artist.localhost/artist/${encodeURIComponent(artist)}?size=${size}`;
}

export function catalogRefreshIsConsistent(
  detectedRevision: string,
  reboundRevision: string,
  snapshotRevision: string,
): boolean {
  return detectedRevision === reboundRevision && detectedRevision === snapshotRevision;
}

export function applyTrackTagProjection(current: Track, updated: Track): Track {
  return {
    ...current,
    rating: updated.rating,
    loved: updated.loved,
    loveState: updated.loveState,
    releaseYear: updated.releaseYear,
    tagSyncState: updated.tagSyncState,
    canUndoTagEdit: updated.canUndoTagEdit,
  };
}

export function applyEditableTrackTagProjection(current: Track, updated: Track): Track {
  return {
    ...applyTrackTagProjection(current, updated),
    title: updated.title,
    artist: updated.artist,
    displayArtist: updated.displayArtist,
    album: updated.album,
    originalYear: updated.originalYear ?? null,
    publisher: updated.publisher ?? null,
    genre: updated.genre,
    trackNumber: updated.trackNumber ?? null,
    trackTotal: updated.trackTotal ?? null,
    discNumber: updated.discNumber ?? null,
    discTotal: updated.discTotal ?? null,
  };
}

type PreviewTrack = Omit<Track, "trackKey" | "loveState" | "tagSyncState" | "canUndoTagEdit">;

function previewTrack(track: PreviewTrack): Track {
  return {
    ...track,
    trackKey: `preview:${track.id}`,
    loveState: track.loved ? "loved" : "neutral",
    tagSyncState: null,
    canUndoTagEdit: false,
  };
}

export interface LibrarySnapshot {
  sourceState: SourceState;
  sourceLabel: string;
  sourcePath: string | null;
  catalogRevision: string;
  summary: LibrarySummary;
  artists: Artist[];
  tracks: Track[];
}

export interface ExplorerCursor {
  value: string;
  id: string;
}

export type TrackSort =
  | "newest"
  | "oldest"
  | "titleAsc"
  | "titleDesc"
  | "artistAsc"
  | "artistDesc"
  | "albumAsc"
  | "albumDesc"
  | "yearAsc"
  | "yearDesc"
  | "releaseYearAsc"
  | "releaseYearDesc"
  | "ratingAsc"
  | "ratingDesc";

export interface TrackPageRequest {
  pageSize?: number;
  cursor?: ExplorerCursor;
  search?: string;
  rating?: number;
  unrated?: boolean;
  loveState?: Track["loveState"];
  yearFrom?: number;
  yearTo?: number;
  yearBasis?: YearBasis;
  missingYear?: boolean;
  genre?: string;
  artist?: string;
  sort?: TrackSort;
}

export interface TrackPage {
  items: Track[];
  nextCursor: ExplorerCursor | null;
  totalCount: number;
}

export interface AlbumSummary {
  id: string;
  title: string;
  artist: string;
  releaseYear: number | null;
  originalYear?: number | null;
  publisher?: string | null;
  originCountryCode?: string | null;
  originCountryName?: string | null;
  genre: string | null;
  totalTracks: number;
  ratedTracks: number;
  lovedTracks: number;
  durationSeconds: number | null;
  rating: number | null;
  albumScore: number | null;
  formats?: string[];
  avgBitrateKbps?: number | null;
}

type ConsistentAlbumValue<T> = { consistent: true; value: T } | { consistent: false };

function consistentAlbumValue<T>(values: readonly T[]): ConsistentAlbumValue<T> {
  const first = values[0];
  return values.every((value) => Object.is(value, first))
    ? { consistent: true, value: first }
    : { consistent: false };
}

export function applyAlbumTrackTagProjection(
  current: AlbumSummary,
  updatedTracks: readonly Track[],
): AlbumSummary {
  if (
    updatedTracks.length !== current.totalTracks
    || updatedTracks.some((track) => track.albumId !== current.id)
  ) {
    return current;
  }

  const title = consistentAlbumValue(updatedTracks.map((track) => track.album));
  const artist = consistentAlbumValue(updatedTracks.map((track) => track.artist));
  const originalYear = consistentAlbumValue(updatedTracks.map((track) => track.originalYear ?? null));
  const releaseYear = consistentAlbumValue(updatedTracks.map((track) => track.releaseYear));
  const publisher = consistentAlbumValue(updatedTracks.map((track) => track.publisher ?? null));
  const genre = consistentAlbumValue(updatedTracks.map((track) => track.genre));
  const projectedArtist = artist.consistent ? artist.value : current.artist;
  const artistChanged = projectedArtist !== current.artist;

  return {
    ...current,
    title: title.consistent ? title.value : current.title,
    artist: projectedArtist,
    originCountryCode: artistChanged ? null : current.originCountryCode,
    originCountryName: artistChanged ? null : current.originCountryName,
    originalYear: originalYear.consistent ? originalYear.value : current.originalYear,
    releaseYear: releaseYear.consistent ? releaseYear.value : current.releaseYear,
    publisher: publisher.consistent ? publisher.value : current.publisher,
    genre: genre.consistent ? genre.value : current.genre,
  };
}

export function applyAlbumTrackMetricsProjection(
  current: AlbumSummary,
  tracks: readonly Track[],
): AlbumSummary {
  if (
    tracks.length !== current.totalTracks
    || tracks.some((track) => track.albumId !== current.id)
  ) {
    return current;
  }

  let ratedTracks = 0;
  let ratingTotal = 0;
  let lovedTracks = 0;
  let fiveStarSeconds = 0;
  let measuredDurationSeconds = 0;
  for (const track of tracks) {
    const durationSeconds = Math.max(0, track.durationSeconds ?? 0);
    measuredDurationSeconds += durationSeconds;
    if (track.rating !== null) {
      ratedTracks += 1;
      ratingTotal += track.rating;
      if (track.rating === 5) fiveStarSeconds += durationSeconds;
    }
    if (track.loved) lovedTracks += 1;
  }

  const rating = ratedTracks > 0 ? ratingTotal / ratedTracks : null;
  const durationSeconds = Math.max(0, current.durationSeconds ?? measuredDurationSeconds);
  const fiveStarRatio = durationSeconds > 0 ? fiveStarSeconds / durationSeconds : 0;
  const albumScore = rating === null
    ? null
    : (((rating * 20 * 0.5) + (fiveStarRatio * 100) + (fiveStarSeconds / 60 * 0.3)) / 10)
      + lovedTracks * 100;

  return {
    ...current,
    ratedTracks,
    lovedTracks,
    rating,
    albumScore,
    genre: consistentAlbumValue(tracks.map((track) => track.genre)).consistent
      ? tracks[0]?.genre ?? null
      : current.genre,
  };
}

export type AlbumSort =
  | "newest"
  | "oldest"
  | "titleAsc"
  | "titleDesc"
  | "artistAsc"
  | "artistDesc"
  | "yearAsc"
  | "yearDesc"
  | "releaseYearAsc"
  | "releaseYearDesc"
  | "ratingAsc"
  | "ratingDesc";

export interface AlbumPageRequest {
  pageSize?: number;
  cursor?: ExplorerCursor;
  search?: string;
  rating?: number;
  unrated?: boolean;
  yearFrom?: number;
  yearTo?: number;
  yearBasis?: YearBasis;
  missingYear?: boolean;
  genre?: string;
  artist?: string;
  sort?: AlbumSort;
}

export interface AlbumPage {
  items: AlbumSummary[];
  nextCursor: ExplorerCursor | null;
  totalCount: number;
}

export type YearBasis = "original" | "release";

export type ArtistSort = "nameAsc" | "nameDesc" | "trackCountAsc" | "trackCountDesc";

export interface ArtistPageRequest {
  pageSize?: number;
  cursor?: ExplorerCursor;
  search?: string;
  genre?: string;
  sort?: ArtistSort;
}

export interface ArtistPage {
  items: Artist[];
  nextCursor: ExplorerCursor | null;
  totalCount: number;
}

export interface AlbumDetail {
  album: AlbumSummary;
  tracks: Track[];
  tracksTruncated: boolean;
  popularity: AlbumPopularity;
}

export interface AlbumPopularity {
  tracks: Array<{ trackKey: string; rank: number }>;
}

export function applyAlbumPopularity(tracks: readonly Track[], popularity: AlbumPopularity): Track[] {
  const ranks = new Map(popularity.tracks.map((track) => [track.trackKey, track.rank]));
  return tracks.map((track) => ({ ...track, lastFmAlbumRank: ranks.get(track.trackKey) ?? null }));
}

export interface TrackDeletionResult {
  deletedTrackKeys: string[];
  failures: Array<{ trackKey: string; title: string; message: string }>;
  catalogSync?: CatalogSync;
}

export interface ArtistDetail {
  artist: Artist;
  albums: AlbumSummary[];
  albumsTruncated: boolean;
}

export const browserPreview: LibrarySnapshot = {
  sourceState: "browser-preview",
  sourceLabel: "Browser preview data",
  sourcePath: null,
  catalogRevision: "0:0:",
  summary: {
    songs: 12_846,
    albums: 1_208,
    artists: 2_302,
    genres: 186,
    loved: 914,
    rated: 4_812,
  },
  artists: [
    { id: "preview-m83", name: "M83", trackCount: 94, albumCount: 9, playCount: 4_218, lastPlayedAtMs: Date.now() - 18 * 60_000 },
    { id: "preview-beethoven", name: "Ludwig van Beethoven", trackCount: 312, albumCount: 28, playCount: 3_804, lastPlayedAtMs: Date.now() - 25 * 60 * 60_000 },
    { id: "preview-daft-punk", name: "Daft Punk", trackCount: 126, albumCount: 12, playCount: 3_116, lastPlayedAtMs: Date.now() - 4 * 24 * 60 * 60_000 },
    { id: "preview-gorillaz", name: "Gorillaz", trackCount: 84, albumCount: 8, playCount: 2_730, lastPlayedAtMs: Date.now() - 8 * 24 * 60 * 60_000 },
    { id: "preview-coldplay", name: "Coldplay", trackCount: 178, albumCount: 14, playCount: 2_414, lastPlayedAtMs: Date.now() - 15 * 24 * 60 * 60_000 },
    { id: "preview-college", name: "College", trackCount: 62, albumCount: 6, playCount: 2_108, lastPlayedAtMs: Date.now() - 31 * 24 * 60 * 60_000 },
    { id: "preview-kavinsky", name: "Kavinsky", trackCount: 47, albumCount: 5, playCount: 1_982, lastPlayedAtMs: Date.now() - 62 * 24 * 60 * 60_000 },
    { id: "preview-the-xx", name: "The xx", trackCount: 53, albumCount: 4, playCount: 1_755, lastPlayedAtMs: Date.now() - 120 * 24 * 60 * 60_000 },
  ],
  tracks: [
    previewTrack({ id: "preview-1", albumId: "preview-hurry-up", title: "Midnight City", artist: "M83", displayArtist: "M83", album: "Hurry Up, We're Dreaming", originalYear: 2011, releaseYear: null, publisher: "Mute Records", originCountryCode: "FR", originCountryName: "France", rating: 5, loved: true, durationSeconds: 243, genre: "Electronic", playCount: 186, trackNumber: 2, trackTotal: 11, discNumber: 1, discTotal: 2 }),
    previewTrack({ id: "preview-8", albumId: "preview-hurry-up", title: "Wait", artist: "M83", displayArtist: "M83", album: "Hurry Up, We're Dreaming", originalYear: 2011, releaseYear: null, publisher: "Mute Records", originCountryCode: "FR", originCountryName: "France", rating: 4.5, loved: false, durationSeconds: 343, genre: "Electronic", playCount: 174, trackNumber: 5, trackTotal: 11, discNumber: 1, discTotal: 2 }),
    previewTrack({ id: "preview-2", albumId: "preview-drive", title: "A Real Hero", artist: "College", displayArtist: "College; Electric Youth", album: "Drive", originalYear: 2011, releaseYear: 2011, publisher: "Lakeshore Records", originCountryCode: "FR", originCountryName: "France", rating: 4, loved: false, durationSeconds: 267, genre: "Soundtrack", playCount: 141 }),
    previewTrack({ id: "preview-3", albumId: "preview-outrun", title: "Nightcall", artist: "Kavinsky", album: "OutRun", originalYear: 2013, releaseYear: 2013, publisher: "Record Makers", originCountryCode: "FR", originCountryName: "France", rating: 4.5, loved: true, durationSeconds: 258, genre: "Synthwave", playCount: 137 }),
    previewTrack({ id: "preview-4", albumId: "preview-xx", title: "Intro", artist: "The xx", album: "xx", originalYear: 2009, releaseYear: 2009, publisher: "Young", originCountryCode: "GB", originCountryName: "United Kingdom", rating: 4, loved: false, durationSeconds: 127, genre: "Indie Rock", playCount: 129 }),
    previewTrack({ id: "preview-5", albumId: "preview-discovery", title: "Digital Love", artist: "Daft Punk", album: "Discovery", originalYear: 2001, releaseYear: 2001, publisher: "Virgin Records", originCountryCode: "FR", originCountryName: "France", rating: 5, loved: true, durationSeconds: 301, genre: "House", playCount: 122 }),
    previewTrack({ id: "preview-6", albumId: "preview-plastic-beach", title: "On Melancholy Hill", artist: "Gorillaz", album: "Plastic Beach", originalYear: 2010, releaseYear: 2010, publisher: "Parlophone", originCountryCode: "GB", originCountryName: "United Kingdom", rating: 4.5, loved: true, durationSeconds: 233, genre: "Alternative", playCount: 116 }),
    previewTrack({ id: "preview-7", albumId: "preview-viva", title: "Strawberry Swing", artist: "Coldplay", album: "Viva la Vida", originalYear: 2008, releaseYear: 2008, publisher: "Parlophone", originCountryCode: "GB", originCountryName: "United Kingdom", rating: 4, loved: false, durationSeconds: 249, genre: "Alternative", playCount: 108 }),
  ],
};

const browserPreviewTrackUpdates = new Map<string, Track>();

export function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

const browserPreviewCoverIds = new Set([
  "preview-viva", "preview-plastic-beach", "preview-discovery", "preview-hurry-up",
  "preview-rainbows", "preview-american-idiot", "preview-drive", "preview-outrun",
  "preview-chart-crowd", "preview-chart-kayleigh", "preview-chart-19", "preview-chart-suddenly",
  "preview-chart-obsession", "preview-chart-view", "preview-chart-fields", "preview-chart-word",
  "preview-chart-crazy", "preview-chart-history", "preview-score-rocky", "preview-score-miami",
  "preview-score-back-future", "preview-score-american-flyers", "preview-score-magnum",
]);

export function albumCoverUrl(albumId: string | null, size: 64 | 128 | 256 | 512): string | null {
  if (!albumId) return null;
  if (!isTauriRuntime()) {
    const previewId = albumId.split(":", 1)[0];
    return browserPreviewCoverIds.has(previewId)
      ? `/__aurora-preview-cover/${encodeURIComponent(previewId)}?size=${size}`
      : null;
  }
  return `http://aurora-cover.localhost/album/${encodeURIComponent(albumId)}?size=${size}`;
}

export async function loadLibrarySnapshot(): Promise<LibrarySnapshot> {
  if (!isTauriRuntime()) {
    return browserPreview;
  }

  return invoke<LibrarySnapshot>("library_snapshot");
}

export async function loadCatalogRevision(): Promise<string> {
  if (!isTauriRuntime()) return browserPreview.catalogRevision;
  return invoke<string>("catalog_revision");
}

export function updateBrowserPreviewTrack(updated: Track): void {
  if (isTauriRuntime()) return;
  browserPreviewTrackUpdates.set(updated.id, updated);
  browserPreview.tracks = browserPreview.tracks.map((track) => track.id === updated.id ? updated : track);
}

export function currentBrowserPreviewTrack(track: Track): Track {
  return isTauriRuntime() ? track : (browserPreviewTrackUpdates.get(track.id) ?? track);
}

export async function loadArtistTracks(artist: string): Promise<Track[]> {
  if (!isTauriRuntime()) {
    return browserPreview.tracks.filter((track) => track.artist === artist);
  }
  return invoke<Track[]>("artist_tracks", { artist });
}

export async function searchLibraryTracks(query: string): Promise<Track[]> {
  if (!isTauriRuntime()) {
    return filterTracks(browserPreview.tracks, query, null);
  }
  return invoke<Track[]>("search_tracks", { query });
}

function browserAlbumSummaries(): AlbumSummary[] {
  const groups = new Map<string, Track[]>();
  for (const track of browserPreview.tracks) {
    if (!track.albumId) continue;
    groups.set(track.albumId, [...(groups.get(track.albumId) ?? []), track]);
  }
  return [...groups.entries()].map(([id, tracks]) => {
    const rated = tracks.filter((track) => track.rating !== null);
    const duration = tracks.some((track) => track.durationSeconds !== null)
      ? tracks.reduce((total, track) => total + (track.durationSeconds ?? 0), 0)
      : null;
    const rating = rated.length === tracks.length && rated.length
      ? rated.reduce((total, track) => total + (track.rating ?? 0), 0) / rated.length
      : null;
    const durationSeconds = duration ?? 0;
    const fiveStarSeconds = tracks.reduce((total, track) => total + (track.rating === 5 ? track.durationSeconds ?? 0 : 0), 0);
    const lovedTracks = tracks.filter((track) => track.loved).length;
    const albumScore = rating === null ? null : (((rating * 20 * .5) + (durationSeconds > 0 ? fiveStarSeconds / durationSeconds * 100 : 0) + (fiveStarSeconds / 60 * .3)) / 10) + lovedTracks * 100;
    return {
      id,
      title: tracks[0].album,
      artist: tracks[0].artist,
      originalYear: tracks[0].originalYear ?? null,
      releaseYear: tracks[0].releaseYear,
      publisher: tracks[0].publisher ?? null,
      originCountryCode: tracks[0].originCountryCode ?? null,
      originCountryName: tracks[0].originCountryName ?? null,
      genre: tracks[0].genre,
      totalTracks: tracks.length,
      ratedTracks: rated.length,
      lovedTracks,
      durationSeconds: duration,
      rating,
      albumScore,
      formats: ["MP3"],
      avgBitrateKbps: 320,
    };
  });
}

function includesExplorerText(values: Array<string | null>, search?: string): boolean {
  const query = search?.trim().toLocaleLowerCase();
  return !query || values.join("\u0000").toLocaleLowerCase().includes(query);
}

function usesAdvancedLibrarySearch(search?: string): boolean {
  return /(?:^|,)\s*-|(?:^|,)\s*(?:artist|aartist|album|genre|year|ryear|publisher|country|title|cr|love)\s*[:=]|(?:^|\s)(?:AND|OR|NOT)(?=\s|$)|"/u.test(search ?? "");
}

function compareText(left: string, right: string, descending = false): number {
  return descending ? right.localeCompare(left) : left.localeCompare(right);
}

function compareNullableNumber(left: number | null | undefined, right: number | null | undefined, descending = false): number {
  if (left == null) return right == null ? 0 : 1;
  if (right == null) return -1;
  return descending ? right - left : left - right;
}

function previewTrackPage(request: TrackPageRequest): TrackPage {
  const yearFor = (track: Track) => request.yearBasis === "release"
    ? track.releaseYear
    : (track.originalYear ?? null);
  const matchingTrackIds = new Set(filterTracks(browserPreview.tracks, request.search ?? "", null).map((track) => track.id));
  const items = browserPreview.tracks
    .filter((track) => matchingTrackIds.has(track.id))
    .filter((track) => request.rating === undefined || track.rating === request.rating)
    .filter((track) => !request.unrated || track.rating === null)
    .filter((track) => request.loveState === undefined || track.loveState === request.loveState)
    .filter((track) => request.yearFrom === undefined || (yearFor(track) !== null && yearFor(track)! >= request.yearFrom))
    .filter((track) => request.yearTo === undefined || (yearFor(track) !== null && yearFor(track)! <= request.yearTo))
    .filter((track) => !request.missingYear || yearFor(track) === null)
    .filter((track) => !request.genre || track.genre === request.genre)
    .filter((track) => !request.artist || track.artist === request.artist)
    .sort((left, right) => {
      switch (request.sort) {
        case "titleAsc": return compareText(left.title, right.title) || compareText(left.id, right.id);
        case "titleDesc": return compareText(left.title, right.title, true) || compareText(left.id, right.id, true);
        case "artistAsc": return compareText(left.artist, right.artist) || compareText(left.title, right.title) || compareText(left.id, right.id);
        case "artistDesc": return compareText(left.artist, right.artist, true) || compareText(left.title, right.title, true) || compareText(left.id, right.id, true);
        case "albumAsc": return compareText(left.album, right.album) || compareText(left.title, right.title) || compareText(left.id, right.id);
        case "albumDesc": return compareText(left.album, right.album, true) || compareText(left.title, right.title, true) || compareText(left.id, right.id, true);
        case "yearAsc": return compareNullableNumber(left.originalYear, right.originalYear) || compareText(left.id, right.id);
        case "yearDesc": return compareNullableNumber(left.originalYear, right.originalYear, true) || compareText(left.id, right.id, true);
        case "releaseYearAsc": return compareNullableNumber(left.releaseYear, right.releaseYear) || compareText(left.id, right.id);
        case "releaseYearDesc": return compareNullableNumber(left.releaseYear, right.releaseYear, true) || compareText(left.id, right.id, true);
        case "ratingAsc": return compareNullableNumber(left.rating, right.rating) || compareText(left.id, right.id);
        case "ratingDesc": return compareNullableNumber(left.rating, right.rating, true) || compareText(left.id, right.id, true);
        case "oldest": return compareText(left.id, right.id);
        default: return compareText(left.id, right.id, true);
      }
    });
  return { items: items.slice(0, request.pageSize ?? 50), nextCursor: null, totalCount: items.length };
}

function previewAlbumPage(request: AlbumPageRequest): AlbumPage {
  const yearFor = (album: AlbumSummary) => request.yearBasis === "release"
    ? album.releaseYear
    : (album.originalYear ?? null);
  const fieldedAlbumIds = usesAdvancedLibrarySearch(request.search)
    ? new Set(filterTracks(browserPreview.tracks, request.search ?? "", null).map((track) => track.albumId))
    : null;
  const items = browserAlbumSummaries()
    .filter((album) => fieldedAlbumIds
      ? fieldedAlbumIds.has(album.id)
      : includesExplorerText([album.title, album.artist, album.genre], request.search))
    .filter((album) => request.rating === undefined || (album.rating !== null && Math.round(album.rating * 2) / 2 === request.rating))
    .filter((album) => !request.unrated || album.rating === null)
    .filter((album) => request.yearFrom === undefined || (yearFor(album) !== null && yearFor(album)! >= request.yearFrom))
    .filter((album) => request.yearTo === undefined || (yearFor(album) !== null && yearFor(album)! <= request.yearTo))
    .filter((album) => !request.missingYear || yearFor(album) === null)
    .filter((album) => !request.genre || album.genre === request.genre)
    .filter((album) => !request.artist || album.artist === request.artist)
    .sort((left, right) => {
      const newestTrackId = (albumId: string) => browserPreview.tracks
        .filter((track) => track.albumId === albumId)
        .reduce((newest, track) => compareText(newest, track.id) < 0 ? track.id : newest, "");
      switch (request.sort) {
        case "newest": return compareText(newestTrackId(left.id), newestTrackId(right.id), true) || compareText(left.id, right.id, true);
        case "oldest": return compareText(newestTrackId(left.id), newestTrackId(right.id)) || compareText(left.id, right.id);
        case "titleAsc": return compareText(left.title, right.title) || compareText(left.id, right.id);
        case "titleDesc": return compareText(left.title, right.title, true) || compareText(left.id, right.id, true);
        case "artistAsc": return compareText(left.artist, right.artist) || compareText(left.title, right.title) || compareText(left.id, right.id);
        case "artistDesc": return compareText(left.artist, right.artist, true) || compareText(left.title, right.title, true) || compareText(left.id, right.id, true);
        case "yearAsc": return compareNullableNumber(left.originalYear, right.originalYear) || compareText(left.id, right.id);
        case "releaseYearAsc": return compareNullableNumber(left.releaseYear, right.releaseYear) || compareText(left.id, right.id);
        case "releaseYearDesc": return compareNullableNumber(left.releaseYear, right.releaseYear, true) || compareText(left.id, right.id, true);
        case "ratingAsc": return compareNullableNumber(left.rating, right.rating) || compareText(left.id, right.id);
        case "ratingDesc": return compareNullableNumber(left.rating, right.rating, true) || compareText(left.id, right.id, true);
        default: return compareNullableNumber(left.originalYear, right.originalYear, true) || compareText(left.id, right.id, true);
      }
    });
  return { items: items.slice(0, request.pageSize ?? 50), nextCursor: null, totalCount: items.length };
}

function previewArtistPage(request: ArtistPageRequest): ArtistPage {
  const genreArtists = request.genre
    ? new Set(browserPreview.tracks.filter((track) => track.genre === request.genre).map((track) => track.artist))
    : null;
  const fieldedArtists = usesAdvancedLibrarySearch(request.search)
    ? new Set(filterTracks(browserPreview.tracks, request.search ?? "", null).map((track) => track.artist))
    : null;
  const items = browserPreview.artists
    .filter((artist) => fieldedArtists
      ? fieldedArtists.has(artist.name)
      : includesExplorerText([artist.name], request.search))
    .filter((artist) => !genreArtists || genreArtists.has(artist.name))
    .sort((left, right) => {
      switch (request.sort) {
        case "nameDesc": return compareText(left.name, right.name, true);
        case "trackCountAsc": return left.trackCount - right.trackCount || compareText(left.name, right.name);
        case "trackCountDesc": return right.trackCount - left.trackCount || compareText(left.name, right.name, true);
        default: return compareText(left.name, right.name);
      }
    });
  return { items: items.slice(0, request.pageSize ?? 50), nextCursor: null, totalCount: items.length };
}

export async function exploreTracks(request: TrackPageRequest): Promise<TrackPage> {
  if (!isTauriRuntime()) return previewTrackPage(request);
  return invoke<TrackPage>("explore_tracks", { request });
}

export async function exploreAlbums(request: AlbumPageRequest): Promise<AlbumPage> {
  if (!isTauriRuntime()) return previewAlbumPage(request);
  return invoke<AlbumPage>("explore_albums", { request });
}

export async function exploreArtists(request: ArtistPageRequest): Promise<ArtistPage> {
  if (!isTauriRuntime()) return previewArtistPage(request);
  return invoke<ArtistPage>("explore_artists", { request });
}

export async function loadAlbumDetail(albumId: string): Promise<AlbumDetail> {
  if (!isTauriRuntime()) {
    const album = browserAlbumSummaries().find((candidate) => candidate.id === albumId);
    if (!album) throw new Error("That album is no longer available.");
    const tracks = browserPreview.tracks.filter((track) => track.albumId === albumId);
    const popularity = { tracks: tracks.filter((track) => track.playCount !== null).sort((a, b) => (b.playCount ?? 0) - (a.playCount ?? 0)).slice(0, 3).map((track, index) => ({ trackKey: track.trackKey, rank: index + 1 })) };
    return { album, tracks: applyAlbumPopularity(tracks, popularity), tracksTruncated: false, popularity };
  }
  return invoke<AlbumDetail>("album_detail", { albumId });
}

export async function loadAlbumPopularity(albumId: string): Promise<AlbumPopularity> {
  if (!isTauriRuntime()) return (await loadAlbumDetail(albumId)).popularity;
  return invoke<AlbumPopularity>("album_popularity", { albumId });
}

export async function deleteAlbumTracks(albumId: string, tracks: readonly Track[]): Promise<TrackDeletionResult> {
  if (tracks.length < 1 || tracks.length > 100) throw new Error("Choose between 1 and 100 album tracks to delete.");
  if (!isTauriRuntime()) {
    const selectedKeys = new Set(tracks.map((track) => track.trackKey));
    const existing = browserPreview.tracks.filter((track) => selectedKeys.has(track.trackKey) && track.albumId === albumId);
    if (existing.length !== tracks.length) throw new Error("One or more selected tracks are no longer available in this album.");
    browserPreview.tracks = browserPreview.tracks.filter((candidate) => !selectedKeys.has(candidate.trackKey));
    for (const track of tracks) browserPreviewTrackUpdates.delete(track.id);
    return {
      deletedTrackKeys: tracks.map((track) => track.trackKey),
      failures: [],
      catalogSync: {
        status: "synced",
        message: `Music Library recorded ${tracks.length} deleted ${tracks.length === 1 ? "track" : "tracks"}.`,
        pendingFolderCount: 0,
        blockedFolderCount: 0,
      },
    };
  }
  return invoke<TrackDeletionResult>("delete_album_track", {
    albumId,
    trackReferences: tracks.map((track) => ({ id: track.id, trackKey: track.trackKey })),
  });
}

export async function loadArtistDetail(artist: string): Promise<ArtistDetail> {
  if (!isTauriRuntime()) {
    const summary = browserPreview.artists.find((candidate) => candidate.name === artist);
    if (!summary) throw new Error("That artist is no longer available.");
    return { artist: summary, albums: browserAlbumSummaries().filter((album) => album.artist === artist), albumsTruncated: false };
  }
  return invoke<ArtistDetail>("artist_detail", { artist });
}

export function formatCount(value: number): string {
  return new Intl.NumberFormat(undefined).format(value);
}

export function formatDuration(seconds: number | null): string {
  if (seconds === null || seconds < 0) return "—";
  const minutes = Math.floor(seconds / 60);
  const remainder = Math.round(seconds % 60);
  return `${minutes}:${remainder.toString().padStart(2, "0")}`;
}

type LibrarySearchField = "any" | "artist" | "aartist" | "album" | "genre" | "year" | "ryear" | "publisher" | "country" | "title" | "cr" | "love";

interface LibrarySearchAlternative {
  field: LibrarySearchField;
  value: string;
  exact: boolean;
  yearFrom: number | null;
  yearTo: number | null;
  numberFrom: number | null;
  numberTo: number | null;
}

interface LibrarySearchGroup {
  negated: boolean;
  alternatives: LibrarySearchAlternative[];
}

type LibrarySearchToken = { kind: "text"; value: string } | { kind: "and" | "or" | "not" };

const librarySearchFields = new Set<LibrarySearchField>([
  "artist", "aartist", "album", "genre", "year", "ryear", "publisher", "country", "title", "cr", "love",
]);

const scoreGenreGroup = new Set([
  "action", "animation", "comedy", "documentary", "drama", "fantasy", "horror",
  "sci-fi", "thriller", "tv", "video game", "western", "anime",
]);

function searchTerms(value: string): string[] {
  return value.toLocaleLowerCase().match(/[\p{L}\p{N}]+/gu) ?? [];
}

function tokenizeLibrarySearch(input: string): LibrarySearchToken[] {
  const characters = Array.from(input);
  const tokens: LibrarySearchToken[] = [];
  let value = "";
  let quoted = false;
  const pushText = () => {
    const text = value.trim();
    if (text) tokens.push({ kind: "text", value: text });
    value = "";
  };

  for (let index = 0; index < characters.length;) {
    const character = characters[index];
    if (character === "\"") {
      quoted = !quoted;
      value += character;
      index += 1;
      continue;
    }
    if (!quoted && character === ",") {
      pushText();
      tokens.push({ kind: "and" });
      index += 1;
      continue;
    }
    const atWordBoundary = index === 0 || /\s/u.test(characters[index - 1]) || characters[index - 1] === ",";
    if (!quoted && atWordBoundary && /[A-Za-z]/u.test(character)) {
      let end = index;
      while (end < characters.length && /[A-Za-z]/u.test(characters[end])) end += 1;
      const boundaryAfter = end === characters.length || /\s/u.test(characters[end]) || characters[end] === ",";
      const word = characters.slice(index, end).join("");
      if (boundaryAfter && (word === "AND" || word === "OR" || word === "NOT")) {
        pushText();
        tokens.push({ kind: word.toLocaleLowerCase() as "and" | "or" | "not" });
        index = end;
        continue;
      }
    }
    value += character;
    index += 1;
  }
  if (quoted) throw new Error("Search quotes are not closed.");
  pushText();
  return tokens;
}

function exactLibrarySearchValue(value: string): string | null {
  const trimmed = value.trim();
  const starts = trimmed.startsWith("\"");
  const ends = trimmed.endsWith("\"");
  if (starts !== ends || (!starts && trimmed.includes("\""))) {
    throw new Error("Quotes must wrap one complete search value.");
  }
  if (!starts) return null;
  const exact = trimmed.slice(1, -1).trim();
  if (!exact) throw new Error("Exact search quotes cannot be empty.");
  return exact;
}

function parseLibrarySearchYearRange(
  value: string,
  field: "year" | "ryear",
): { yearFrom: number | null; yearTo: number | null } {
  const parts = value.trim().split("..");
  if (parts.length > 2) {
    throw new Error(`${field} range must use one '..', for example ${field}:1985..1987.`);
  }
  const parseBound = (bound: string): number | null => {
    const trimmed = bound.trim();
    if (!trimmed) return null;
    if (!/^\d{4}$/u.test(trimmed) || Number(trimmed) < 1000 || Number(trimmed) > 2999) {
      throw new Error(`${field} must be a year between 1000 and 2999.`);
    }
    return Number(trimmed);
  };
  if (parts.length === 1) {
    const year = parseBound(parts[0]);
    if (year === null) throw new Error(`${field} must be a year between 1000 and 2999.`);
    return { yearFrom: year, yearTo: year };
  }
  const yearFrom = parseBound(parts[0]);
  const yearTo = parseBound(parts[1]);
  if (yearFrom === null && yearTo === null) {
    throw new Error(`${field} range needs a starting or ending year.`);
  }
  if (yearFrom !== null && yearTo !== null && yearFrom > yearTo) {
    throw new Error(`${field} range must start at or before its ending year.`);
  }
  return { yearFrom, yearTo };
}

function parseLibrarySearchNumberRange(
  value: string,
  field: "cr" | "love",
): { numberFrom: number | null; numberTo: number | null } {
  const parts = value.trim().split("..");
  if (parts.length > 2) {
    throw new Error(`${field} range must use one '..', for example ${field}:1..3.`);
  }
  const invalid = field === "cr"
    ? "cr bounds must be whole percentages from 0 through 100."
    : "love bounds must be non-negative whole track counts.";
  const parseBound = (bound: string): number | null => {
    const trimmed = bound.trim();
    if (!trimmed) return null;
    if (!/^\d+$/u.test(trimmed)) throw new Error(invalid);
    const number = Number(trimmed);
    if (!Number.isSafeInteger(number) || (field === "cr" && number > 100) || number > 4_294_967_295) {
      throw new Error(invalid);
    }
    return number;
  };
  if (parts.length === 1) {
    const number = parseBound(parts[0]);
    if (number === null) throw new Error(invalid);
    if (field === "cr") return { numberFrom: 0, numberTo: number };
    if (number === 0) return { numberFrom: 0, numberTo: 0 };
    if (number === 1) return { numberFrom: 1, numberTo: null };
    throw new Error("love must be 0, 1, or an inclusive range such as love:1..3.");
  }
  const numberFrom = parseBound(parts[0]);
  const numberTo = parseBound(parts[1]);
  if (numberFrom === null && numberTo === null) {
    throw new Error(`${field} range needs a starting or ending number.`);
  }
  if (numberFrom !== null && numberTo !== null && numberFrom > numberTo) {
    throw new Error(`${field} range must start at or before its ending number.`);
  }
  return { numberFrom, numberTo };
}

function parseLibrarySearch(query: string): LibrarySearchGroup[] {
  const groups: LibrarySearchGroup[] = [];
  let current: LibrarySearchGroup | null = null;
  let inheritedField: LibrarySearchField | null = null;
  let pendingNot = false;
  let afterOr = false;
  let termCount = 0;
  let alternativeCount = 0;

  for (const token of tokenizeLibrarySearch(query)) {
    if (token.kind === "text") {
      let raw = token.value.trim();
      const negativePrefix = raw.startsWith("-");
      if (negativePrefix) {
        raw = raw.slice(1).trim();
        if (!raw) throw new Error("Negative search needs a value after '-'.");
        if (current) throw new Error("Use a comma, AND, or NOT before a negative '-' clause.");
      }
      if (!current) {
        current = { negated: pendingNot || negativePrefix, alternatives: [] };
        pendingNot = false;
      }
      const colonSeparator = raw.indexOf(":");
      const equalsSeparator = raw.indexOf("=");
      const separator = colonSeparator < 0
        ? equalsSeparator
        : equalsSeparator < 0
          ? colonSeparator
          : Math.min(colonSeparator, equalsSeparator);
      const candidateField = separator >= 0
        ? raw.slice(0, separator).trim().toLocaleLowerCase()
        : "";
      const explicitField = librarySearchFields.has(candidateField as LibrarySearchField);
      const field: LibrarySearchField = explicitField
        ? candidateField as LibrarySearchField
        : (inheritedField ?? "any");
      const value = explicitField ? raw.slice(separator + 1).trim() : raw;
      if (explicitField) inheritedField = field;
      if (!value) throw new Error("Search field needs a value.");
      const exact = exactLibrarySearchValue(value);
      let yearFrom: number | null = null;
      let yearTo: number | null = null;
      let numberFrom: number | null = null;
      let numberTo: number | null = null;
      if (field === "year" || field === "ryear") {
        ({ yearFrom, yearTo } = parseLibrarySearchYearRange(exact ?? value, field));
      } else if (field === "cr" || field === "love") {
        ({ numberFrom, numberTo } = parseLibrarySearchNumberRange(exact ?? value, field));
      } else if (exact === null) {
        termCount += searchTerms(value).length;
        if (termCount > 32) throw new Error("Search can contain at most 32 words.");
        if (searchTerms(value).length === 0) throw new Error("Search needs a word or an exact quoted value.");
      }
      alternativeCount += 1;
      if (alternativeCount > 32) throw new Error("Search can contain at most 32 alternatives.");
      current.alternatives.push({
        field,
        value: exact ?? value,
        exact: exact !== null,
        yearFrom,
        yearTo,
        numberFrom,
        numberTo,
      });
      afterOr = false;
      continue;
    }
    if (token.kind === "or") {
      if (!current || current.alternatives.length === 0 || afterOr) {
        throw new Error("OR needs a search value on both sides.");
      }
      afterOr = true;
      continue;
    }
    if (afterOr) throw new Error(token.kind === "not" ? "NOT cannot replace a value after OR." : "OR needs a search value on both sides.");
    if (current) groups.push(current);
    current = null;
    inheritedField = null;
    if (token.kind === "not") {
      if (pendingNot) throw new Error("NOT needs one search clause.");
      pendingNot = true;
    } else {
      pendingNot = false;
    }
  }
  if (afterOr) throw new Error("OR needs a search value on both sides.");
  if (pendingNot) throw new Error("NOT needs one search clause.");
  if (current) groups.push(current);
  return groups;
}

function librarySearchValues(track: Track, field: LibrarySearchField): string[] {
  switch (field) {
    case "artist": return [track.displayArtist ?? track.artist];
    case "aartist": return [track.artist];
    case "album": return [track.album];
    case "genre": return [track.genre ?? ""];
    case "publisher": return [track.publisher ?? ""];
    case "country": return [track.originCountryName ?? "", track.originCountryCode ?? ""];
    case "title": return [track.title];
    case "cr":
    case "love": return [];
    case "year":
    case "ryear": return [];
    default: return [
      track.title,
      track.displayArtist ?? track.artist,
      track.artist,
      track.album,
      track.genre ?? "",
      track.publisher ?? "",
    ];
  }
}

interface AlbumSearchStats {
  totalTracks: number;
  ratedTracks: number;
  lovedTracks: number;
}

function albumSearchStats(tracks: readonly Track[]): Map<string, AlbumSearchStats> {
  const stats = new Map<string, AlbumSearchStats>();
  for (const track of tracks) {
    if (!track.albumId) continue;
    const album = stats.get(track.albumId) ?? { totalTracks: 0, ratedTracks: 0, lovedTracks: 0 };
    album.totalTracks += 1;
    if (track.rating !== null) album.ratedTracks += 1;
    if (track.loved) album.lovedTracks += 1;
    stats.set(track.albumId, album);
  }
  return stats;
}

function matchesLibrarySearchAlternative(
  track: Track,
  alternative: LibrarySearchAlternative,
  albumStats: ReadonlyMap<string, AlbumSearchStats>,
): boolean {
  if (alternative.field === "year" || alternative.field === "ryear") {
    const year = alternative.field === "year" ? track.originalYear : track.releaseYear;
    if (year === null || year === undefined) return false;
    return (alternative.yearFrom === null || year >= alternative.yearFrom)
      && (alternative.yearTo === null || year <= alternative.yearTo);
  }
  if (alternative.field === "cr") {
    const album = track.albumId ? albumStats.get(track.albumId) : undefined;
    if (!album || album.totalTracks <= 0) return false;
    const completeness = album.ratedTracks / album.totalTracks * 100;
    return (alternative.numberFrom === null || completeness >= alternative.numberFrom)
      && (alternative.numberTo === null || completeness <= alternative.numberTo);
  }
  if (alternative.field === "love") {
    const album = track.albumId ? albumStats.get(track.albumId) : undefined;
    if (!album) return false;
    return (alternative.numberFrom === null || album.lovedTracks >= alternative.numberFrom)
      && (alternative.numberTo === null || album.lovedTracks <= alternative.numberTo);
  }
  const values = librarySearchValues(track, alternative.field);
  if (
    alternative.field === "genre"
    && !alternative.exact
    && (alternative.value.toLocaleLowerCase() === "score" || alternative.value.toLocaleLowerCase() === "scores")
  ) {
    return values.some((value) => scoreGenreGroup.has(value.trim().toLocaleLowerCase()));
  }
  if (alternative.exact) {
    const exact = alternative.value.trim().toLocaleLowerCase();
    return values.some((value) => value.trim().toLocaleLowerCase() === exact);
  }
  const queryTerms = searchTerms(alternative.value);
  return queryTerms.every((term) => values.some((value) => (
    searchTerms(value).some((valueTerm) => valueTerm.startsWith(term))
  )));
}

export function filterTracks(tracks: Track[], query: string, artist: string | null): Track[] {
  const normalized = query.trim();
  const groups = normalized ? parseLibrarySearch(normalized) : [];
  const albumStats = albumSearchStats(tracks);

  return tracks.filter((track) => {
    if (artist && track.artist !== artist) return false;
    return !normalized || groups.every((group) => {
      const matched = group.alternatives.some((alternative) => (
        matchesLibrarySearchAlternative(track, alternative, albumStats)
      ));
      return group.negated ? !matched : matched;
    });
  });
}
