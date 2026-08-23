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
  displayArtist?: string;
  album: string;
  releaseYear: number | null;
  originalYear?: number | null;
  publisher?: string | null;
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

export type TrackSort = "newest" | "titleAsc" | "artistAsc" | "albumAsc" | "yearDesc" | "releaseYearDesc" | "ratingDesc";

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
}

export interface AlbumSummary {
  id: string;
  title: string;
  artist: string;
  releaseYear: number | null;
  originalYear?: number | null;
  genre: string | null;
  totalTracks: number;
  ratedTracks: number;
  lovedTracks: number;
  durationSeconds: number | null;
  rating: number | null;
  albumScore: number | null;
}

export type AlbumSort = "titleAsc" | "artistAsc" | "yearDesc" | "releaseYearDesc" | "ratingDesc";

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
}

export type YearBasis = "original" | "release";

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
    previewTrack({ id: "preview-1", albumId: "preview-hurry-up", title: "Midnight City", artist: "M83", album: "Hurry Up, We're Dreaming", originalYear: 2011, releaseYear: null, rating: 5, loved: true, durationSeconds: 243, genre: "Electronic", playCount: 186 }),
    previewTrack({ id: "preview-2", albumId: "preview-drive", title: "A Real Hero", artist: "College", album: "Drive", originalYear: 2011, releaseYear: 2011, rating: 4, loved: false, durationSeconds: 267, genre: "Soundtrack", playCount: 141 }),
    previewTrack({ id: "preview-3", albumId: "preview-outrun", title: "Nightcall", artist: "Kavinsky", album: "OutRun", originalYear: 2013, releaseYear: 2013, rating: 4.5, loved: true, durationSeconds: 258, genre: "Synthwave", playCount: 137 }),
    previewTrack({ id: "preview-4", albumId: "preview-xx", title: "Intro", artist: "The xx", album: "xx", originalYear: 2009, releaseYear: 2009, rating: 4, loved: false, durationSeconds: 127, genre: "Indie Rock", playCount: 129 }),
    previewTrack({ id: "preview-5", albumId: "preview-discovery", title: "Digital Love", artist: "Daft Punk", album: "Discovery", originalYear: 2001, releaseYear: 2001, rating: 5, loved: true, durationSeconds: 301, genre: "House", playCount: 122 }),
    previewTrack({ id: "preview-6", albumId: "preview-plastic-beach", title: "On Melancholy Hill", artist: "Gorillaz", album: "Plastic Beach", originalYear: 2010, releaseYear: 2010, rating: 4.5, loved: true, durationSeconds: 233, genre: "Alternative", playCount: 116 }),
    previewTrack({ id: "preview-7", albumId: "preview-viva", title: "Strawberry Swing", artist: "Coldplay", album: "Viva la Vida", originalYear: 2008, releaseYear: 2008, rating: 4, loved: false, durationSeconds: 249, genre: "Alternative", playCount: 108 }),
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
      genre: tracks[0].genre,
      totalTracks: tracks.length,
      ratedTracks: rated.length,
      lovedTracks,
      durationSeconds: duration,
      rating,
      albumScore,
    };
  });
}

function includesExplorerText(values: Array<string | null>, search?: string): boolean {
  const query = search?.trim().toLocaleLowerCase();
  return !query || values.join("\u0000").toLocaleLowerCase().includes(query);
}

function usesAdvancedLibrarySearch(search?: string): boolean {
  return /(?:^|,)\s*-|(?:^|,)\s*(?:artist|aartist|album|genre|year|ryear|publisher|title)\s*:|(?:^|\s)(?:AND|OR|NOT)(?=\s|$)|"/u.test(search ?? "");
}

function previewTrackPage(request: TrackPageRequest): TrackPage {
  const yearFor = (track: Track) => request.yearBasis === "release"
    ? track.releaseYear
    : (track.originalYear ?? null);
  const items = browserPreview.tracks
    .filter((track) => filterTracks([track], request.search ?? "", null).length > 0)
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
        case "titleAsc": return left.title.localeCompare(right.title) || left.id.localeCompare(right.id);
        case "artistAsc": return left.artist.localeCompare(right.artist) || left.title.localeCompare(right.title);
        case "albumAsc": return left.album.localeCompare(right.album) || left.title.localeCompare(right.title);
        case "yearDesc": return (right.originalYear ?? -1) - (left.originalYear ?? -1) || left.title.localeCompare(right.title);
        case "releaseYearDesc": return (right.releaseYear ?? -1) - (left.releaseYear ?? -1) || left.title.localeCompare(right.title);
        case "ratingDesc": return (right.rating ?? -1) - (left.rating ?? -1) || left.title.localeCompare(right.title);
        default: return right.id.localeCompare(left.id);
      }
    });
  return { items: items.slice(0, request.pageSize ?? 50), nextCursor: null };
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
      switch (request.sort) {
        case "titleAsc": return left.title.localeCompare(right.title) || left.id.localeCompare(right.id);
        case "artistAsc": return left.artist.localeCompare(right.artist) || left.title.localeCompare(right.title);
        case "ratingDesc": return (right.rating ?? -1) - (left.rating ?? -1) || left.title.localeCompare(right.title);
        case "releaseYearDesc": return (right.releaseYear ?? -1) - (left.releaseYear ?? -1) || left.title.localeCompare(right.title);
        default: return (right.originalYear ?? -1) - (left.originalYear ?? -1) || left.title.localeCompare(right.title);
      }
    });
  return { items: items.slice(0, request.pageSize ?? 50), nextCursor: null };
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

type LibrarySearchField = "any" | "artist" | "aartist" | "album" | "genre" | "year" | "ryear" | "publisher" | "title";

interface LibrarySearchAlternative {
  field: LibrarySearchField;
  value: string;
  exact: boolean;
  yearFrom: number | null;
  yearTo: number | null;
}

interface LibrarySearchGroup {
  negated: boolean;
  alternatives: LibrarySearchAlternative[];
}

type LibrarySearchToken = { kind: "text"; value: string } | { kind: "and" | "or" | "not" };

const librarySearchFields = new Set<LibrarySearchField>([
  "artist", "aartist", "album", "genre", "year", "ryear", "publisher", "title",
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
      const separator = raw.indexOf(":");
      const candidateField = separator >= 0 ? raw.slice(0, separator).trim().toLocaleLowerCase() : "";
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
      if (field === "year" || field === "ryear") {
        ({ yearFrom, yearTo } = parseLibrarySearchYearRange(exact ?? value, field));
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
    case "title": return [track.title];
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

function matchesLibrarySearchAlternative(track: Track, alternative: LibrarySearchAlternative): boolean {
  if (alternative.field === "year" || alternative.field === "ryear") {
    const year = alternative.field === "year" ? track.originalYear : track.releaseYear;
    if (year === null || year === undefined) return false;
    return (alternative.yearFrom === null || year >= alternative.yearFrom)
      && (alternative.yearTo === null || year <= alternative.yearTo);
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

  return tracks.filter((track) => {
    if (artist && track.artist !== artist) return false;
    return !normalized || groups.every((group) => {
      const matched = group.alternatives.some((alternative) => (
        matchesLibrarySearchAlternative(track, alternative)
      ));
      return group.negated ? !matched : matched;
    });
  });
}
