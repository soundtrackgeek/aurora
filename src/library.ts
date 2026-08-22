import { invoke } from "@tauri-apps/api/core";

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
}

export interface Track {
  id: string;
  trackKey: string;
  albumId: string | null;
  title: string;
  artist: string;
  album: string;
  releaseYear: number | null;
  rating: number | null;
  loved: boolean;
  loveState: "neutral" | "loved" | "banned";
  tagSyncState: "pendingImport" | null;
  canUndoTagEdit: boolean;
  durationSeconds: number | null;
  genre: string | null;
  playCount: number | null;
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
  summary: LibrarySummary;
  artists: Artist[];
  tracks: Track[];
}

export interface ExplorerCursor {
  value: string;
  id: string;
}

export type TrackSort = "newest" | "titleAsc" | "artistAsc" | "albumAsc" | "releaseYearDesc" | "ratingDesc";

export interface TrackPageRequest {
  pageSize?: number;
  cursor?: ExplorerCursor;
  search?: string;
  rating?: number;
  unrated?: boolean;
  loveState?: Track["loveState"];
  yearFrom?: number;
  yearTo?: number;
  genre?: string;
  artist?: string;
  sort?: TrackSort;
}

export interface TrackPage {
  items: Track[];
  nextCursor: ExplorerCursor | null;
}

export interface AlbumSummary {
  id: string;
  title: string;
  artist: string;
  releaseYear: number | null;
  genre: string | null;
  totalTracks: number;
  ratedTracks: number;
  lovedTracks: number;
  durationSeconds: number | null;
  rating: number | null;
}

export type AlbumSort = "titleAsc" | "artistAsc" | "releaseYearDesc" | "ratingDesc";

export interface AlbumPageRequest {
  pageSize?: number;
  cursor?: ExplorerCursor;
  search?: string;
  yearFrom?: number;
  yearTo?: number;
  genre?: string;
  artist?: string;
  sort?: AlbumSort;
}

export interface AlbumPage {
  items: AlbumSummary[];
  nextCursor: ExplorerCursor | null;
}

export type ArtistSort = "nameAsc" | "trackCountDesc";

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
}

export interface AlbumDetail {
  album: AlbumSummary;
  tracks: Track[];
  tracksTruncated: boolean;
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
  summary: {
    songs: 12_846,
    albums: 1_208,
    artists: 2_302,
    genres: 186,
    loved: 914,
    rated: 4_812,
  },
  artists: [
    { id: "preview-m83", name: "M83", trackCount: 94, albumCount: 9, playCount: 4_218 },
    { id: "preview-beethoven", name: "Ludwig van Beethoven", trackCount: 312, albumCount: 28, playCount: 3_804 },
    { id: "preview-daft-punk", name: "Daft Punk", trackCount: 126, albumCount: 12, playCount: 3_116 },
    { id: "preview-gorillaz", name: "Gorillaz", trackCount: 84, albumCount: 8, playCount: 2_730 },
    { id: "preview-coldplay", name: "Coldplay", trackCount: 178, albumCount: 14, playCount: 2_414 },
    { id: "preview-college", name: "College", trackCount: 62, albumCount: 6, playCount: 2_108 },
    { id: "preview-kavinsky", name: "Kavinsky", trackCount: 47, albumCount: 5, playCount: 1_982 },
    { id: "preview-the-xx", name: "The xx", trackCount: 53, albumCount: 4, playCount: 1_755 },
  ],
  tracks: [
    previewTrack({ id: "preview-1", albumId: "preview-hurry-up", title: "Midnight City", artist: "M83", album: "Hurry Up, We're Dreaming", releaseYear: 2011, rating: 5, loved: true, durationSeconds: 243, genre: "Electronic", playCount: 186 }),
    previewTrack({ id: "preview-2", albumId: "preview-drive", title: "A Real Hero", artist: "College", album: "Drive", releaseYear: 2011, rating: 4, loved: false, durationSeconds: 267, genre: "Soundtrack", playCount: 141 }),
    previewTrack({ id: "preview-3", albumId: "preview-outrun", title: "Nightcall", artist: "Kavinsky", album: "OutRun", releaseYear: 2013, rating: 4.5, loved: true, durationSeconds: 258, genre: "Synthwave", playCount: 137 }),
    previewTrack({ id: "preview-4", albumId: "preview-xx", title: "Intro", artist: "The xx", album: "xx", releaseYear: 2009, rating: 4, loved: false, durationSeconds: 127, genre: "Indie Rock", playCount: 129 }),
    previewTrack({ id: "preview-5", albumId: "preview-discovery", title: "Digital Love", artist: "Daft Punk", album: "Discovery", releaseYear: 2001, rating: 5, loved: true, durationSeconds: 301, genre: "House", playCount: 122 }),
    previewTrack({ id: "preview-6", albumId: "preview-plastic-beach", title: "On Melancholy Hill", artist: "Gorillaz", album: "Plastic Beach", releaseYear: 2010, rating: 4.5, loved: true, durationSeconds: 233, genre: "Alternative", playCount: 116 }),
    previewTrack({ id: "preview-7", albumId: "preview-viva", title: "Strawberry Swing", artist: "Coldplay", album: "Viva la Vida", releaseYear: 2008, rating: 4, loved: false, durationSeconds: 249, genre: "Alternative", playCount: 108 }),
  ],
};

export function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export function albumCoverUrl(albumId: string | null, size: 64 | 128 | 256 | 512): string | null {
  if (!albumId || !isTauriRuntime()) return null;
  return `http://aurora-cover.localhost/album/${encodeURIComponent(albumId)}?size=${size}`;
}

export async function loadLibrarySnapshot(): Promise<LibrarySnapshot> {
  if (!isTauriRuntime()) {
    return browserPreview;
  }

  return invoke<LibrarySnapshot>("library_snapshot");
}

export function updateBrowserPreviewTrack(updated: Track): void {
  if (isTauriRuntime()) return;
  browserPreview.tracks = browserPreview.tracks.map((track) => track.id === updated.id ? updated : track);
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
    return {
      id,
      title: tracks[0].album,
      artist: tracks[0].artist,
      releaseYear: tracks[0].releaseYear,
      genre: tracks[0].genre,
      totalTracks: tracks.length,
      ratedTracks: rated.length,
      lovedTracks: tracks.filter((track) => track.loved).length,
      durationSeconds: duration,
      rating: rated.length ? rated.reduce((total, track) => total + (track.rating ?? 0), 0) / rated.length : null,
    };
  });
}

function includesExplorerText(values: Array<string | null>, search?: string): boolean {
  const query = search?.trim().toLocaleLowerCase();
  return !query || values.join("\u0000").toLocaleLowerCase().includes(query);
}

function previewTrackPage(request: TrackPageRequest): TrackPage {
  const items = browserPreview.tracks
    .filter((track) => includesExplorerText([track.title, track.artist, track.album, track.genre], request.search))
    .filter((track) => request.rating === undefined || track.rating === request.rating)
    .filter((track) => !request.unrated || track.rating === null)
    .filter((track) => request.loveState === undefined || track.loveState === request.loveState)
    .filter((track) => request.yearFrom === undefined || (track.releaseYear !== null && track.releaseYear >= request.yearFrom))
    .filter((track) => request.yearTo === undefined || (track.releaseYear !== null && track.releaseYear <= request.yearTo))
    .filter((track) => !request.genre || track.genre === request.genre)
    .filter((track) => !request.artist || track.artist === request.artist)
    .sort((left, right) => {
      switch (request.sort) {
        case "titleAsc": return left.title.localeCompare(right.title) || left.id.localeCompare(right.id);
        case "artistAsc": return left.artist.localeCompare(right.artist) || left.title.localeCompare(right.title);
        case "albumAsc": return left.album.localeCompare(right.album) || left.title.localeCompare(right.title);
        case "releaseYearDesc": return (right.releaseYear ?? -1) - (left.releaseYear ?? -1) || left.title.localeCompare(right.title);
        case "ratingDesc": return (right.rating ?? -1) - (left.rating ?? -1) || left.title.localeCompare(right.title);
        default: return right.id.localeCompare(left.id);
      }
    });
  return { items: items.slice(0, request.pageSize ?? 50), nextCursor: null };
}

function previewAlbumPage(request: AlbumPageRequest): AlbumPage {
  const items = browserAlbumSummaries()
    .filter((album) => includesExplorerText([album.title, album.artist, album.genre], request.search))
    .filter((album) => request.yearFrom === undefined || (album.releaseYear !== null && album.releaseYear >= request.yearFrom))
    .filter((album) => request.yearTo === undefined || (album.releaseYear !== null && album.releaseYear <= request.yearTo))
    .filter((album) => !request.genre || album.genre === request.genre)
    .filter((album) => !request.artist || album.artist === request.artist)
    .sort((left, right) => {
      switch (request.sort) {
        case "titleAsc": return left.title.localeCompare(right.title) || left.id.localeCompare(right.id);
        case "artistAsc": return left.artist.localeCompare(right.artist) || left.title.localeCompare(right.title);
        case "ratingDesc": return (right.rating ?? -1) - (left.rating ?? -1) || left.title.localeCompare(right.title);
        default: return (right.releaseYear ?? -1) - (left.releaseYear ?? -1) || left.title.localeCompare(right.title);
      }
    });
  return { items: items.slice(0, request.pageSize ?? 50), nextCursor: null };
}

function previewArtistPage(request: ArtistPageRequest): ArtistPage {
  const genreArtists = request.genre
    ? new Set(browserPreview.tracks.filter((track) => track.genre === request.genre).map((track) => track.artist))
    : null;
  const items = browserPreview.artists
    .filter((artist) => includesExplorerText([artist.name], request.search))
    .filter((artist) => !genreArtists || genreArtists.has(artist.name))
    .sort((left, right) => request.sort === "trackCountDesc"
      ? right.trackCount - left.trackCount || left.name.localeCompare(right.name)
      : left.name.localeCompare(right.name));
  return { items: items.slice(0, request.pageSize ?? 50), nextCursor: null };
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
    return { album, tracks: browserPreview.tracks.filter((track) => track.albumId === albumId), tracksTruncated: false };
  }
  return invoke<AlbumDetail>("album_detail", { albumId });
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

export function filterTracks(tracks: Track[], query: string, artist: string | null): Track[] {
  const normalized = query.trim().toLocaleLowerCase();
  return tracks.filter((track) => {
    if (artist && track.artist !== artist) return false;
    if (!normalized) return true;
    return [track.title, track.artist, track.album, track.genre ?? ""]
      .join("\u0000")
      .toLocaleLowerCase()
      .includes(normalized);
  });
}
