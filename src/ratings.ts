import { invoke } from "@tauri-apps/api/core";
import { browserPreview, currentBrowserPreviewTrack, isTauriRuntime, loadAlbumDetail, type Track } from "./library";

export type RatingMode = "tracks" | "albums";
export type CompletionKind = "almostComplete" | "partiallyRated" | "unrated";

export interface RatingBand {
  rating: number | null;
  count: number;
}

export interface CompletionCounts {
  almostComplete: number;
  partiallyRated: number;
  unrated: number;
}

export interface RatingAlbum {
  id: string;
  title: string;
  artist: string;
  originalYear: number | null;
  releaseYear: number | null;
  publisher?: string | null;
  genre: string | null;
  totalTracks: number;
  ratedTracks: number;
  lovedTracks: number;
  durationSeconds: number;
  remainingTracks: number;
  effectiveRating: number | null;
  provisionalRating: number | null;
  albumScore: number | null;
}

export interface RatingAlbumPage {
  kind: CompletionKind;
  total: number;
  albums: RatingAlbum[];
}

export interface RatingsOverview {
  trackBands: RatingBand[];
  albumBands: RatingBand[];
  completion: CompletionCounts;
  ratedAlbums: number;
  fiveStarAlbums: RatingAlbum[];
  initialPage: RatingAlbumPage;
}

const trackCounts = new Map<number | null, number>([
  [null, 947_794], [0.5, 0], [1, 324], [1.5, 0], [2, 2_387], [2.5, 0],
  [3, 25_142], [3.5, 1], [4, 61_346], [4.5, 1], [5, 59_293],
]);

const albumCounts = new Map<number | null, number>([
  [null, 59_578], [0.5, 0], [1, 12], [1.5, 8], [2, 68], [2.5, 85],
  [3, 1_298], [3.5, 2_618], [4, 3_847], [4.5, 2_716], [5, 1_782],
]);

function spectrum(source: Map<number | null, number>): RatingBand[] {
  return [null, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4, 4.5, 5]
    .map((rating) => ({ rating, count: source.get(rating) ?? 0 }));
}

const PREVIEW_ALBUMS: readonly RatingAlbum[] = [
  { id: "preview-viva", title: "Viva La Vida", artist: "Coldplay", originalYear: 2008, releaseYear: 2008, genre: "Alternative", totalTracks: 10, ratedTracks: 8, lovedTracks: 5, durationSeconds: 2779, remainingTracks: 2, effectiveRating: null, provisionalRating: 4.5, albumScore: null },
  { id: "preview-plastic-beach", title: "Plastic Beach", artist: "Gorillaz", originalYear: 2010, releaseYear: 2010, genre: "Alternative", totalTracks: 16, ratedTracks: 14, lovedTracks: 3, durationSeconds: 3371, remainingTracks: 2, effectiveRating: null, provisionalRating: 4.2, albumScore: null },
  { id: "preview-discovery", title: "Discovery", artist: "Daft Punk", originalYear: 2001, releaseYear: 2001, genre: "House", totalTracks: 14, ratedTracks: 12, lovedTracks: 6, durationSeconds: 3660, remainingTracks: 2, effectiveRating: null, provisionalRating: 4.7, albumScore: null },
  { id: "preview-hurry-up", title: "Hurry Up, We're Dreaming", artist: "M83", originalYear: 2011, releaseYear: 2011, genre: "Electronic", totalTracks: 22, ratedTracks: 20, lovedTracks: 7, durationSeconds: 4402, remainingTracks: 2, effectiveRating: null, provisionalRating: 4.4, albumScore: null },
  { id: "preview-rainbows", title: "In Rainbows", artist: "Radiohead", originalYear: 2007, releaseYear: 2007, genre: "Alternative Rock", totalTracks: 10, ratedTracks: 7, lovedTracks: 5, durationSeconds: 2554, remainingTracks: 3, effectiveRating: null, provisionalRating: 4.8, albumScore: null },
  { id: "preview-outrun", title: "OutRun", artist: "Kavinsky", originalYear: 2013, releaseYear: 2013, genre: "Synthwave", totalTracks: 13, ratedTracks: 10, lovedTracks: 2, durationSeconds: 3544, remainingTracks: 3, effectiveRating: null, provisionalRating: 4.1, albumScore: null },
  { id: "preview-american-idiot", title: "American Idiot", artist: "Green Day", originalYear: 2004, releaseYear: 2004, genre: "Alternative Rock", totalTracks: 13, ratedTracks: 10, lovedTracks: 4, durationSeconds: 3434, remainingTracks: 3, effectiveRating: null, provisionalRating: 4.3, albumScore: null },
];

function previewAlbumTracks(album: RatingAlbum): Track[] {
  const known = browserPreview.tracks.filter((track) => track.albumId === album.id);
  const generated = Array.from({ length: album.totalTracks }, (_, index): Track => {
    const titles = album.id === "preview-viva"
      ? ["Life in Technicolor", "Cemeteries of London", "Lost!", "42", "Lovers in Japan", "Yes", "Viva La Vida", "Violet Hill", "Strawberry Swing", "Death and All His Friends"]
      : [];
    const rated = index < album.ratedTracks;
    const loved = index < album.lovedTracks;
    return {
      id: `ratings:${album.id}:${index + 1}`,
      trackKey: `preview:ratings:${album.id}:${index + 1}`,
      albumId: album.id,
      title: titles[index] ?? `${album.title} · Track ${index + 1}`,
      artist: album.artist,
      album: album.title,
      originalYear: album.originalYear,
      releaseYear: album.releaseYear,
      publisher: album.publisher ?? null,
      rating: rated ? Math.max(3, 5 - (index % 4) * 0.5) : null,
      loved,
      loveState: loved ? "loved" : "neutral",
      tagSyncState: null,
      canUndoTagEdit: false,
      durationSeconds: Math.round(album.durationSeconds / album.totalTracks),
      genre: album.genre,
      playCount: null,
    };
  });
  return (known.length >= generated.length ? known : generated).map(currentBrowserPreviewTrack);
}

const PREVIEW_COMPLETION: CompletionCounts = {
  almostComplete: 678,
  partiallyRated: 5_723,
  unrated: 59_578,
};

function previewPage(kind: CompletionKind): RatingAlbumPage {
  const albums = kind === "almostComplete"
    ? [...PREVIEW_ALBUMS]
    : PREVIEW_ALBUMS.map((album, index) => kind === "partiallyRated"
      ? (() => {
        const ratedTracks = Math.min(album.totalTracks - 4, Math.max(1, 2 + index));
        return { ...album, id: `${album.id}:partial`, ratedTracks, remainingTracks: album.totalTracks - ratedTracks, provisionalRating: 3.5 + index * 0.15 };
      })()
      : { ...album, id: `${album.id}:unrated`, ratedTracks: 0, lovedTracks: 0, remainingTracks: album.totalTracks, provisionalRating: null });
  if (kind === "unrated") {
    albums.sort((left, right) => (right.originalYear ?? -1) - (left.originalYear ?? -1) || left.title.localeCompare(right.title));
  }
  return { kind, total: PREVIEW_COMPLETION[kind], albums: albums.map(syncPreviewAlbum) };
}

function syncPreviewAlbum(album: RatingAlbum): RatingAlbum {
  const tracks = previewAlbumTracks(album);
  const rated = tracks.filter((track) => track.rating !== null);
  const lovedTracks = tracks.filter((track) => track.loved).length;
  const provisionalRating = rated.length
    ? rated.reduce((total, track) => total + (track.rating ?? 0), 0) / rated.length
    : null;
  const effectiveRating = rated.length === album.totalTracks && rated.length > 0
    ? Math.round((provisionalRating ?? 0) * 20) / 20
    : null;
  const fiveStarSeconds = tracks.reduce((total, track) => total + (track.rating === 5 ? track.durationSeconds ?? 0 : 0), 0);
  const ratio = album.durationSeconds > 0 ? fiveStarSeconds / album.durationSeconds : 0;
  const albumScore = effectiveRating === null
    ? null
    : (((effectiveRating * 20 * .5) + (ratio * 100) + (fiveStarSeconds / 60 * .3)) / 10) + lovedTracks * 100;
  return {
    ...album,
    ratedTracks: rated.length,
    lovedTracks,
    remainingTracks: Math.max(0, album.totalTracks - rated.length),
    provisionalRating,
    effectiveRating,
    albumScore,
  };
}

function previewOverview(): RatingsOverview {
  return {
    trackBands: spectrum(trackCounts),
    albumBands: spectrum(albumCounts),
    completion: PREVIEW_COMPLETION,
    ratedAlbums: 12_434,
    fiveStarAlbums: PREVIEW_ALBUMS.map((album) => ({ ...album, ratedTracks: album.totalTracks, remainingTracks: 0, effectiveRating: 5, provisionalRating: 5, albumScore: 510 + album.lovedTracks * 100 })),
    initialPage: previewPage("almostComplete"),
  };
}

export async function loadRatingsOverview(): Promise<RatingsOverview> {
  if (!isTauriRuntime()) return previewOverview();
  return invoke<RatingsOverview>("ratings_overview");
}

export async function loadRatingAlbumPage(kind: CompletionKind): Promise<RatingAlbumPage> {
  if (!isTauriRuntime()) return previewPage(kind);
  return invoke<RatingAlbumPage>("rating_album_page", { kind });
}

export async function loadRatingAlbumTracks(album: RatingAlbum): Promise<Track[]> {
  if (!isTauriRuntime()) return previewAlbumTracks(album);
  return (await loadAlbumDetail(album.id)).tracks;
}

export async function loadRatingCollection(mode: RatingMode, rating: number | null, limit = 100): Promise<Track[]> {
  if (!isTauriRuntime()) {
    if (mode === "tracks") {
      const matching = browserPreview.tracks.filter((track) => track.rating === rating);
      return (matching.length ? matching : PREVIEW_ALBUMS.flatMap(previewAlbumTracks).filter((track) => track.rating === rating)).slice(0, limit);
    }
    const albums = previewOverview().fiveStarAlbums.filter((album) => album.effectiveRating === rating);
    return albums.flatMap(previewAlbumTracks).slice(0, limit);
  }
  return invoke<Track[]>("rating_collection_tracks", { request: { mode, rating, limit } });
}

export async function loadRatingAlbumQueue(album: RatingAlbum, unratedOnly = true, limit = 100): Promise<Track[]> {
  if (!isTauriRuntime()) {
    const tracks = previewAlbumTracks(album);
    return (unratedOnly ? tracks.filter((track) => track.rating === null) : tracks).slice(0, limit);
  }
  return invoke<Track[]>("rating_album_queue_tracks", { request: { albumId: album.id, unratedOnly, limit } });
}
