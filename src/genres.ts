import { invoke } from "@tauri-apps/api/core";
import { browserPreview, isTauriRuntime, type Track } from "./library";

export interface GenreSummary {
  name: string;
  trackCount: number;
  albumCount: number;
  artistCount: number;
  ratedTracks: number;
  lovedTracks: number;
  durationSeconds: number;
  averageRating: number | null;
  firstYear: number | null;
  lastYear: number | null;
  representativeAlbumId: string | null;
  sessions: number;
  plays: number;
  listenedSeconds: number;
  lastListenedAtMs: number | null;
}

export interface GenreDecade {
  decade: number;
  trackCount: number;
  albumCount: number;
}

export interface GenreAlbum {
  id: string;
  title: string;
  artist: string;
  year: number | null;
  publisher?: string | null;
  totalTracks: number;
  ratedTracks: number;
  lovedTracks: number;
  durationSeconds: number;
  rating: number | null;
}

export interface GenreArtist {
  name: string;
  trackCount: number;
  albumCount: number;
  lovedTracks: number;
}

export interface RelatedGenre {
  name: string;
  sharedArtists: number;
  sharedAlbums: number;
  sharedTracks: number;
}

export interface GenreDetail {
  summary: GenreSummary;
  decades: GenreDecade[];
  albums: GenreAlbum[];
  artists: GenreArtist[];
  relatedGenres: RelatedGenre[];
  highlights: Track[];
}

export type GenreSort = "size" | "rating" | "loved" | "recent" | "unexplored" | "alphabetical";
export type GenreQueueMode = "radio" | "shuffle" | "loved" | "highestRated" | "rediscover" | "unrated";

export interface GenreQueueRequest {
  genre: string;
  mode: GenreQueueMode;
  limit: number;
  excludeTrackKeys: string[];
}

export interface GenreRadioSession {
  version: 1;
  genre: string;
  mode: GenreQueueMode;
}

const GENRE_RADIO_STORAGE_KEY = "aurora.genre-radio.v1";

function genreTracks(name: string): Track[] {
  return browserPreview.tracks.filter((track) => track.genre === name);
}

function previewAlbumGroups(tracks: readonly Track[]): GenreAlbum[] {
  const groups = new Map<string, Track[]>();
  for (const track of tracks) {
    if (!track.albumId) continue;
    groups.set(track.albumId, [...(groups.get(track.albumId) ?? []), track]);
  }
  return [...groups.entries()].map(([id, albumTracks]) => {
    const rated = albumTracks.filter((track) => track.rating !== null);
    return {
      id,
      title: albumTracks[0].album,
      artist: albumTracks[0].artist,
      year: albumTracks[0].originalYear ?? null,
      publisher: albumTracks[0].publisher ?? null,
      totalTracks: albumTracks.length,
      ratedTracks: rated.length,
      lovedTracks: albumTracks.filter((track) => track.loved).length,
      durationSeconds: albumTracks.reduce((total, track) => total + (track.durationSeconds ?? 0), 0),
      rating: rated.length
        ? rated.reduce((total, track) => total + (track.rating ?? 0), 0) / rated.length
        : null,
    };
  });
}

function previewSummary(name: string): GenreSummary {
  const tracks = genreTracks(name);
  const albums = previewAlbumGroups(tracks);
  const years = tracks.flatMap((track) => track.originalYear == null ? [] : [track.originalYear]);
  const ratings = tracks.flatMap((track) => track.rating === null ? [] : [track.rating]);
  return {
    name,
    trackCount: tracks.length,
    albumCount: albums.length,
    artistCount: new Set(tracks.map((track) => track.artist)).size,
    ratedTracks: ratings.length,
    lovedTracks: tracks.filter((track) => track.loved).length,
    durationSeconds: tracks.reduce((total, track) => total + (track.durationSeconds ?? 0), 0),
    averageRating: ratings.length ? ratings.reduce((total, rating) => total + rating, 0) / ratings.length : null,
    firstYear: years.length ? Math.min(...years) : null,
    lastYear: years.length ? Math.max(...years) : null,
    representativeAlbumId: albums.sort((left, right) => right.lovedTracks - left.lovedTracks || (right.rating ?? -1) - (left.rating ?? -1))[0]?.id ?? null,
    sessions: 0,
    plays: 0,
    listenedSeconds: 0,
    lastListenedAtMs: null,
  };
}

const previewRelations: Record<string, Array<[string, number]>> = {
  Alternative: [["Indie Rock", 42], ["Electronic", 18], ["Pop", 11]],
  Electronic: [["Synthwave", 38], ["House", 26], ["Alternative", 18]],
  House: [["Electronic", 26], ["Synthwave", 9], ["Pop", 7]],
  "Indie Rock": [["Alternative", 42], ["Pop", 14], ["Electronic", 6]],
  Soundtrack: [["Synthwave", 21], ["Electronic", 16], ["Alternative", 8]],
  Synthwave: [["Electronic", 38], ["Soundtrack", 21], ["House", 9]],
};

function previewGenreNames(): string[] {
  return [...new Set(browserPreview.tracks.flatMap((track) => track.genre ? [track.genre] : []))];
}

let genreNamesRequest: Promise<string[]> | null = null;

export function loadGenreNames(): Promise<string[]> {
  if (!genreNamesRequest) {
    genreNamesRequest = (isTauriRuntime()
      ? invoke<string[]>("genre_names")
      : Promise.resolve(previewGenreNames().sort((left, right) => left.localeCompare(right))))
      .catch((error) => {
        genreNamesRequest = null;
        throw error;
      });
  }
  return genreNamesRequest;
}

function previewGenreIndex(): GenreSummary[] {
  return previewGenreNames()
    .map(previewSummary)
    .sort((left, right) => right.trackCount - left.trackCount || left.name.localeCompare(right.name));
}

function previewGenreDetail(name: string): GenreDetail {
  const tracks = genreTracks(name);
  if (tracks.length === 0) throw new Error("That genre is no longer available in the catalog.");
  const albums = previewAlbumGroups(tracks)
    .sort((left, right) => right.lovedTracks - left.lovedTracks || (right.rating ?? -1) - (left.rating ?? -1));
  const artists = [...new Set(tracks.map((track) => track.artist))]
    .map((artist) => {
      const artistTracks = tracks.filter((track) => track.artist === artist);
      return {
        name: artist,
        trackCount: artistTracks.length,
        albumCount: new Set(artistTracks.map((track) => track.albumId)).size,
        lovedTracks: artistTracks.filter((track) => track.loved).length,
      };
    })
    .sort((left, right) => right.trackCount - left.trackCount || left.name.localeCompare(right.name));
  const decadeMap = new Map<number, { tracks: number; albums: Set<string> }>();
  for (const track of tracks) {
    if (track.originalYear == null) continue;
    const decade = Math.floor(track.originalYear / 10) * 10;
    const current = decadeMap.get(decade) ?? { tracks: 0, albums: new Set<string>() };
    current.tracks += 1;
    if (track.albumId) current.albums.add(track.albumId);
    decadeMap.set(decade, current);
  }
  const known = new Set(previewGenreNames());
  return {
    summary: previewSummary(name),
    decades: [...decadeMap.entries()]
      .map(([decade, value]) => ({ decade, trackCount: value.tracks, albumCount: value.albums.size }))
      .sort((left, right) => left.decade - right.decade),
    albums,
    artists,
    relatedGenres: (previewRelations[name] ?? [])
      .filter(([related]) => known.has(related))
      .map(([related, sharedArtists]) => ({
        name: related,
        sharedArtists,
        sharedAlbums: Math.max(1, Math.round(sharedArtists * 1.8)),
        sharedTracks: Math.max(1, sharedArtists * 12),
      })),
    highlights: [...tracks].sort((left, right) => Number(right.loved) - Number(left.loved) || (right.rating ?? -1) - (left.rating ?? -1)),
  };
}

function stableTrackOrder(track: Track): number {
  let hash = 2166136261;
  for (const character of track.trackKey) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function previewQueue(request: GenreQueueRequest): Track[] {
  const excluded = new Set(request.excludeTrackKeys);
  let tracks = genreTracks(request.genre).filter((track) => !excluded.has(track.trackKey));
  if (request.mode === "loved") tracks = tracks.filter((track) => track.loved);
  if (request.mode === "highestRated" || request.mode === "rediscover") tracks = tracks.filter((track) => track.rating !== null);
  if (request.mode === "unrated") tracks = tracks.filter((track) => track.rating === null);
  tracks.sort((left, right) => {
    if (request.mode === "highestRated" || request.mode === "rediscover") {
      return (right.rating ?? -1) - (left.rating ?? -1) || stableTrackOrder(left) - stableTrackOrder(right);
    }
    if (request.mode === "radio") {
      return Number(right.loved) - Number(left.loved)
        || (right.rating ?? -1) - (left.rating ?? -1)
        || stableTrackOrder(left) - stableTrackOrder(right);
    }
    return stableTrackOrder(left) - stableTrackOrder(right);
  });
  return tracks.slice(0, request.limit);
}

export async function loadGenreIndex(): Promise<GenreSummary[]> {
  if (!isTauriRuntime()) return previewGenreIndex();
  return invoke<GenreSummary[]>("genre_index");
}

export async function loadGenreDetail(genre: string): Promise<GenreDetail> {
  if (!isTauriRuntime()) return previewGenreDetail(genre);
  return invoke<GenreDetail>("genre_detail", { genre });
}

export async function loadGenreQueue(request: GenreQueueRequest): Promise<Track[]> {
  if (!isTauriRuntime()) return previewQueue(request);
  return invoke<Track[]>("genre_queue_tracks", { request });
}

export function loadGenreRadioSession(): GenreRadioSession | null {
  try {
    const parsed = JSON.parse(window.localStorage.getItem(GENRE_RADIO_STORAGE_KEY) ?? "null") as Partial<GenreRadioSession> | null;
    if (!parsed || parsed.version !== 1 || typeof parsed.genre !== "string" || !isGenreQueueMode(parsed.mode)) return null;
    return { version: 1, genre: parsed.genre, mode: parsed.mode };
  } catch {
    return null;
  }
}

export function saveGenreRadioSession(session: GenreRadioSession | null): void {
  try {
    if (session) window.localStorage.setItem(GENRE_RADIO_STORAGE_KEY, JSON.stringify(session));
    else window.localStorage.removeItem(GENRE_RADIO_STORAGE_KEY);
  } catch {
    // Playback still works when the webview blocks device-local preferences.
  }
}

function isGenreQueueMode(value: unknown): value is GenreQueueMode {
  return value === "radio" || value === "shuffle" || value === "loved"
    || value === "highestRated" || value === "rediscover" || value === "unrated";
}

export function sortGenres(genres: readonly GenreSummary[], sort: GenreSort): GenreSummary[] {
  return [...genres].sort((left, right) => {
    switch (sort) {
      case "rating":
        return (right.averageRating ?? -1) - (left.averageRating ?? -1) || right.ratedTracks - left.ratedTracks || left.name.localeCompare(right.name);
      case "loved":
        return right.lovedTracks - left.lovedTracks || right.trackCount - left.trackCount || left.name.localeCompare(right.name);
      case "recent":
        return (right.lastListenedAtMs ?? -1) - (left.lastListenedAtMs ?? -1) || right.plays - left.plays || left.name.localeCompare(right.name);
      case "unexplored":
        return left.plays - right.plays || left.ratedTracks - right.ratedTracks || right.trackCount - left.trackCount || left.name.localeCompare(right.name);
      case "alphabetical":
        return left.name.localeCompare(right.name);
      default:
        return right.trackCount - left.trackCount || left.name.localeCompare(right.name);
    }
  });
}
