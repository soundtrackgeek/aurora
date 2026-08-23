import { invoke } from "@tauri-apps/api/core";
import {
  browserPreview,
  isTauriRuntime,
  loadAlbumDetail,
  type Track,
} from "./library";

export type PublisherTimelineMode = "release" | "original" | "share";

export interface PublisherActivityBucket {
  year: number;
  albumCount: number;
  trackCount: number;
}

export interface PublisherSummary {
  name: string;
  albumCount: number;
  trackCount: number;
  firstYear: number | null;
  lastYear: number | null;
  releaseActivity: PublisherActivityBucket[];
  originalActivity: PublisherActivityBucket[];
  logoUrl: string | null;
}

export interface PublisherAlbum {
  id: string;
  title: string;
  artist: string;
  originalYear: number | null;
  releaseYear: number | null;
  publisher: string;
  totalTracks: number;
  ratedTracks: number;
  lovedTracks: number;
  durationSeconds: number;
  genre: string | null;
  rating: number | null;
}

export interface PublisherDetail {
  publisher: PublisherSummary;
  albums: PublisherAlbum[];
}

export interface PublisherOverview {
  publishers: PublisherSummary[];
  initialDetail: PublisherDetail;
}

const PUBLISHER_PREVIEW_META = [
  { name: "Parlophone", albumCount: 284, trackCount: 4_659, firstYear: 1958, lastYear: 2026, peak: 1966, seed: 11 },
  { name: "ECM Records", albumCount: 251, trackCount: 3_184, firstYear: 1969, lastYear: 2026, peak: 2001, seed: 23 },
  { name: "Warp Records", albumCount: 184, trackCount: 2_772, firstYear: 1989, lastYear: 2026, peak: 2016, seed: 37 },
  { name: "Motown", albumCount: 234, trackCount: 3_818, firstYear: 1959, lastYear: 2026, peak: 1972, seed: 41 },
  { name: "Blue Note", albumCount: 212, trackCount: 3_021, firstYear: 1956, lastYear: 2026, peak: 1964, seed: 53 },
  { name: "Deutsche Grammophon", albumCount: 296, trackCount: 4_395, firstYear: 1951, lastYear: 2026, peak: 1981, seed: 67 },
] as const;

function previewActivity(
  firstYear: number,
  lastYear: number,
  peak: number,
  seed: number,
  multiplier = 1,
): PublisherActivityBucket[] {
  const buckets: PublisherActivityBucket[] = [];
  for (let year = 1950; year <= 2026; year += 2) {
    if (year < firstYear || year > lastYear) {
      buckets.push({ year, albumCount: 0, trackCount: 0 });
      continue;
    }
    const spread = Math.max(18, (lastYear - firstYear) * .48);
    const distance = Math.abs(year - peak) / spread;
    const shape = Math.max(.08, 1 - distance * .72);
    const ripple = .72 + ((year * seed) % 17) / 30;
    const albumCount = Math.max(1, Math.round(shape * ripple * 20 * multiplier));
    buckets.push({ year, albumCount, trackCount: albumCount * (11 + seed % 7) });
  }
  return buckets;
}

const PREVIEW_PUBLISHERS: PublisherSummary[] = PUBLISHER_PREVIEW_META.map((publisher, index) => ({
  ...publisher,
  releaseActivity: previewActivity(
    publisher.firstYear,
    publisher.lastYear,
    publisher.peak,
    publisher.seed,
    1 + index * .07,
  ),
  originalActivity: previewActivity(
    publisher.firstYear,
    publisher.lastYear,
    publisher.peak - 4,
    publisher.seed + 9,
    .82 + index * .05,
  ),
  logoUrl: null,
}));

const PREVIEW_ALBUMS: readonly PublisherAlbum[] = [
  { id: "preview-chart-crowd", title: "Come Fly with Me", artist: "Frank Sinatra", originalYear: 1958, releaseYear: 1958, publisher: "Parlophone", totalTracks: 12, ratedTracks: 12, lovedTracks: 4, durationSeconds: 2780, genre: "Vocal Jazz", rating: 5 },
  { id: "preview-rainbows", title: "Revolver", artist: "The Beatles", originalYear: 1966, releaseYear: 1966, publisher: "Parlophone", totalTracks: 14, ratedTracks: 14, lovedTracks: 8, durationSeconds: 2107, genre: "Rock", rating: 5 },
  { id: "preview-score-rocky", title: "The Dark Side of the Moon", artist: "Pink Floyd", originalYear: 1973, releaseYear: 1973, publisher: "Parlophone", totalTracks: 10, ratedTracks: 10, lovedTracks: 9, durationSeconds: 2583, genre: "Progressive Rock", rating: 5 },
  { id: "preview-viva", title: "Hounds of Love", artist: "Kate Bush", originalYear: 1985, releaseYear: 1985, publisher: "Parlophone", totalTracks: 12, ratedTracks: 12, lovedTracks: 7, durationSeconds: 2854, genre: "Art Pop", rating: 5 },
  { id: "preview-chart-fields", title: "OK Computer", artist: "Radiohead", originalYear: 1997, releaseYear: 1997, publisher: "Parlophone", totalTracks: 12, ratedTracks: 12, lovedTracks: 8, durationSeconds: 3213, genre: "Alternative Rock", rating: 5 },
  { id: "preview-discovery", title: "Parachutes", artist: "Coldplay", originalYear: 2000, releaseYear: 2000, publisher: "Parlophone", totalTracks: 10, ratedTracks: 10, lovedTracks: 5, durationSeconds: 2498, genre: "Alternative", rating: 4.5 },
  { id: "preview-plastic-beach", title: "Plastic Beach", artist: "Gorillaz", originalYear: 2010, releaseYear: 2010, publisher: "Parlophone", totalTracks: 16, ratedTracks: 14, lovedTracks: 6, durationSeconds: 3371, genre: "Alternative", rating: 4.5 },
  { id: "preview-american-idiot", title: "Seventeen Going Under", artist: "Sam Fender", originalYear: 2021, releaseYear: 2021, publisher: "Parlophone", totalTracks: 11, ratedTracks: 9, lovedTracks: 3, durationSeconds: 3158, genre: "Indie Rock", rating: 4.5 },
  { id: "preview-hurry-up", title: "Selected Ambient Works", artist: "Aphex Twin", originalYear: 1992, releaseYear: 1992, publisher: "Warp Records", totalTracks: 13, ratedTracks: 13, lovedTracks: 7, durationSeconds: 4450, genre: "Ambient", rating: 5 },
  { id: "preview-outrun", title: "Cosmogramma", artist: "Flying Lotus", originalYear: 2010, releaseYear: 2010, publisher: "Warp Records", totalTracks: 17, ratedTracks: 15, lovedTracks: 5, durationSeconds: 2735, genre: "Electronic", rating: 4.5 },
];

function previewAlbumsFor(publisher: string): PublisherAlbum[] {
  const matching = PREVIEW_ALBUMS.filter((album) => album.publisher === publisher);
  if (matching.length) return [...matching].sort((left, right) => {
    if (left.id === "preview-plastic-beach") return -1;
    if (right.id === "preview-plastic-beach") return 1;
    return (left.releaseYear ?? 0) - (right.releaseYear ?? 0);
  });
  return PREVIEW_ALBUMS.slice(0, 6).map((album, index) => ({
    ...album,
    id: `${album.id}:${publisher}:${index}`,
    publisher,
  }));
}

function previewDetail(publisher: PublisherSummary): PublisherDetail {
  return { publisher, albums: previewAlbumsFor(publisher.name) };
}

function previewOverview(search?: string): PublisherOverview {
  const query = search?.trim().toLocaleLowerCase();
  const matches = query
    ? PREVIEW_PUBLISHERS.filter((publisher) => publisher.name.toLocaleLowerCase().includes(query))
    : PREVIEW_PUBLISHERS;
  const publishers = matches.length ? matches : PREVIEW_PUBLISHERS;
  return { publishers, initialDetail: previewDetail(publishers[0]) };
}

function previewTracks(album: PublisherAlbum): Track[] {
  const known = browserPreview.tracks.filter((track) => track.albumId === album.id);
  if (known.length) return known.map((track) => ({ ...track, publisher: album.publisher }));
  return Array.from({ length: Math.min(album.totalTracks, 12) }, (_, index) => ({
    id: `publisher:${album.id}:${index + 1}`,
    trackKey: `preview:publisher:${album.id}:${index + 1}`,
    albumId: album.id,
    title: `${album.title} · Track ${index + 1}`,
    artist: album.artist,
    album: album.title,
    originalYear: album.originalYear,
    releaseYear: album.releaseYear,
    publisher: album.publisher,
    rating: index < album.ratedTracks ? album.rating : null,
    loved: index < album.lovedTracks,
    loveState: index < album.lovedTracks ? "loved" : "neutral",
    tagSyncState: null,
    canUndoTagEdit: false,
    durationSeconds: Math.round(album.durationSeconds / Math.max(1, album.totalTracks)),
    genre: album.genre,
    playCount: null,
  }));
}

export async function loadPublisherOverview(search?: string): Promise<PublisherOverview> {
  if (!isTauriRuntime()) return previewOverview(search);
  return invoke<PublisherOverview>("publisher_overview", { search: search?.trim() || null });
}

export async function loadPublisherDetail(publisher: string): Promise<PublisherDetail> {
  if (!isTauriRuntime()) {
    const summary = PREVIEW_PUBLISHERS.find((candidate) => candidate.name === publisher)
      ?? PREVIEW_PUBLISHERS[0];
    return previewDetail(summary);
  }
  return invoke<PublisherDetail>("publisher_detail", { publisher });
}

export async function loadPublisherQueue(publisher: string, limit = 100): Promise<Track[]> {
  if (!isTauriRuntime()) return previewAlbumsFor(publisher).flatMap(previewTracks).slice(0, limit);
  return invoke<Track[]>("publisher_queue_tracks", { request: { publisher, limit } });
}

export async function loadPublisherAlbumTracks(album: PublisherAlbum): Promise<Track[]> {
  if (!isTauriRuntime()) return previewTracks(album);
  return (await loadAlbumDetail(album.id)).tracks;
}
