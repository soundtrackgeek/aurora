import { invoke } from "@tauri-apps/api/core";
import { browserPreview, isTauriRuntime, loadAlbumDetail, type Track, type YearBasis } from "./library";

export type YearsMode = "release" | "original" | "twoClocks";

export interface YearSelection {
  basis: YearBasis;
  year: number | null;
}

export interface YearBucket {
  year: number;
  albumCount: number;
  trackCount: number;
  ratedTracks: number;
  lovedTracks: number;
}

export interface YearStats {
  firstYear: number | null;
  lastYear: number | null;
  differentAlbums: number;
  differentTracks: number;
  missingOriginalAlbums: number;
  missingOriginalTracks: number;
  missingReleaseAlbums: number;
  missingReleaseTracks: number;
}

export interface YearSummary {
  albumCount: number;
  trackCount: number;
  ratedTracks: number;
  lovedTracks: number;
  durationSeconds: number;
}

export interface YearFlow {
  year: number | null;
  albumCount: number;
  trackCount: number;
}

export interface YearAlbum {
  id: string;
  title: string;
  artist: string;
  originalYear: number | null;
  releaseYear: number | null;
  publisher?: string | null;
  totalTracks: number;
  ratedTracks: number;
  lovedTracks: number;
  durationSeconds: number;
  genre: string | null;
  rating: number | null;
}

export interface YearDetail {
  selection: YearSelection;
  summary: YearSummary;
  flows: YearFlow[];
  albums: YearAlbum[];
}

export interface YearOverview {
  originalYears: YearBucket[];
  releaseYears: YearBucket[];
  stats: YearStats;
  initialDetail: YearDetail;
}

const PREVIEW_ALBUMS: readonly YearAlbum[] = [
  { id: "year-blade-runner", title: "Blade Runner (Expanded Edition)", artist: "Vangelis", originalYear: 1982, releaseYear: 2025, publisher: "East West Records", totalTracks: 32, ratedTracks: 27, lovedTracks: 5, durationSeconds: 4542, genre: "Stage & Screen", rating: 5 },
  { id: "year-thriller", title: "Thriller", artist: "Michael Jackson", originalYear: 1982, releaseYear: 1982, totalTracks: 9, ratedTracks: 9, lovedTracks: 7, durationSeconds: 2539, genre: "Contemporary R&B", rating: 5 },
  { id: "year-poltergeist", title: "Poltergeist", artist: "Jerry Goldsmith", originalYear: 1982, releaseYear: 2010, totalTracks: 35, ratedTracks: 28, lovedTracks: 9, durationSeconds: 8379, genre: "Horror", rating: 5 },
  { id: "year-conan", title: "Conan the Barbarian", artist: "Basil Poledouris", originalYear: 1982, releaseYear: 2012, totalTracks: 53, ratedTracks: 53, lovedTracks: 5, durationSeconds: 11215, genre: "Fantasy", rating: 4.5 },
  { id: "year-first-blood", title: "First Blood", artist: "Jerry Goldsmith", originalYear: 1982, releaseYear: 2025, totalTracks: 34, ratedTracks: 28, lovedTracks: 4, durationSeconds: 5495, genre: "Action", rating: 5 },
  { id: "year-number-beast", title: "The Number of the Beast", artist: "Iron Maiden", originalYear: 1982, releaseYear: 1982, totalTracks: 8, ratedTracks: 8, lovedTracks: 2, durationSeconds: 2427, genre: "Heavy Metal", rating: 5 },
  { id: "year-miles", title: "Miles Davis at the Blackhawk", artist: "Miles Davis", originalYear: 1961, releaseYear: 2025, totalTracks: 18, ratedTracks: 10, lovedTracks: 2, durationSeconds: 5820, genre: "Jazz", rating: 4.5 },
  { id: "year-pet-sounds", title: "Pet Sounds Sessions", artist: "The Beach Boys", originalYear: 1966, releaseYear: 2025, totalTracks: 42, ratedTracks: 20, lovedTracks: 3, durationSeconds: 7040, genre: "Pop", rating: 4.5 },
  { id: "year-bends", title: "The Bends (30th Anniversary)", artist: "Radiohead", originalYear: 1995, releaseYear: 2025, totalTracks: 24, ratedTracks: 18, lovedTracks: 4, durationSeconds: 5100, genre: "Alternative Rock", rating: 5 },
  { id: "year-nine-inch", title: "The Fragile: Definitive Edition", artist: "Nine Inch Nails", originalYear: 1999, releaseYear: 2025, totalTracks: 31, ratedTracks: 22, lovedTracks: 3, durationSeconds: 6200, genre: "Industrial Rock", rating: 4.5 },
  { id: "year-daft", title: "Human After All (20th Anniversary)", artist: "Daft Punk", originalYear: 2005, releaseYear: 2025, totalTracks: 15, ratedTracks: 12, lovedTracks: 2, durationSeconds: 3600, genre: "Electronic", rating: 4.5 },
  { id: "year-archive", title: "Signals from the Archive", artist: "Aurora Ensemble", originalYear: null, releaseYear: 2025, totalTracks: 12, ratedTracks: 0, lovedTracks: 0, durationSeconds: 2800, genre: "Ambient", rating: null },
];

function previewTimeline(basis: YearBasis): YearBucket[] {
  const result: YearBucket[] = [];
  for (let year = 1946; year <= 2026; year += 1) {
    const ageShape = Math.max(0, 1 - Math.abs(year - (basis === "original" ? 1981 : 1998)) / 55);
    const recentLift = basis === "release" && year >= 2021 ? (year - 2020) * 0.33 : 0;
    const ripple = ((year * 17) % 13) / 16;
    const albumCount = Math.max(1, Math.round(25 + 760 * ageShape * (0.42 + ripple) + 230 * recentLift));
    const trackCount = Math.round(albumCount * (basis === "release" ? 22.7 : 15.4));
    result.push({
      year,
      albumCount,
      trackCount,
      ratedTracks: Math.round(trackCount * 0.37),
      lovedTracks: Math.round(trackCount * 0.012),
    });
  }
  return result;
}

function previewSummary(albums: readonly YearAlbum[]): YearSummary {
  return albums.reduce<YearSummary>((summary, album) => ({
    albumCount: summary.albumCount + 1,
    trackCount: summary.trackCount + album.totalTracks,
    ratedTracks: summary.ratedTracks + album.ratedTracks,
    lovedTracks: summary.lovedTracks + album.lovedTracks,
    durationSeconds: summary.durationSeconds + album.durationSeconds,
  }), { albumCount: 0, trackCount: 0, ratedTracks: 0, lovedTracks: 0, durationSeconds: 0 });
}

function previewDetail(selection: YearSelection): YearDetail {
  const albums = PREVIEW_ALBUMS.filter((album) => {
    const value = selection.basis === "original" ? album.originalYear : album.releaseYear;
    return selection.year === null ? value === null : value === selection.year;
  });
  const visibleAlbums = albums.length ? albums : PREVIEW_ALBUMS.slice(0, 6).map((album, index) => ({
    ...album,
    id: `${album.id}:${selection.basis}:${selection.year}:${index}`,
    originalYear: selection.basis === "original" ? selection.year : album.originalYear,
    releaseYear: selection.basis === "release" ? selection.year : album.releaseYear,
  }));
  const counterpart = new Map<number | null, YearFlow>();
  for (const album of visibleAlbums) {
    const year = selection.basis === "original" ? album.releaseYear : album.originalYear;
    const current = counterpart.get(year) ?? { year, albumCount: 0, trackCount: 0 };
    current.albumCount += 1;
    current.trackCount += album.totalTracks;
    counterpart.set(year, current);
  }
  const summary = previewSummary(visibleAlbums);
  if (selection.basis === "release" && selection.year === 2025) {
    Object.assign(summary, { albumCount: 2_606, trackCount: 59_620, ratedTracks: 24_829, lovedTracks: 234, durationSeconds: 15_882_120 });
  } else if (selection.basis === "original" && selection.year === 1982) {
    Object.assign(summary, { albumCount: 413, trackCount: 11_814, ratedTracks: 7_401, lovedTracks: 612, durationSeconds: 3_120_500 });
  }
  return {
    selection,
    summary,
    flows: [...counterpart.values()].sort((left, right) => (left.year ?? 9999) - (right.year ?? 9999)),
    albums: visibleAlbums,
  };
}

const PREVIEW_OVERVIEW: YearOverview = {
  originalYears: previewTimeline("original"),
  releaseYears: previewTimeline("release"),
  stats: {
    firstYear: 1946,
    lastYear: 2026,
    differentAlbums: 9_385,
    differentTracks: 209_671,
    missingOriginalAlbums: 172,
    missingOriginalTracks: 1_887,
    missingReleaseAlbums: 37_016,
    missingReleaseTracks: 422_009,
  },
  initialDetail: previewDetail({ basis: "release", year: 2025 }),
};

function previewTracksForAlbum(album: YearAlbum): Track[] {
  return Array.from({ length: Math.min(8, album.totalTracks) }, (_, index) => ({
    id: `year-track:${album.id}:${index + 1}`,
    trackKey: `preview:year-track:${album.id}:${index + 1}`,
    albumId: album.id,
    title: index === 0 && album.id === "year-blade-runner" ? "Tears in Rain (2025 Edit)" : `${album.title} · Track ${index + 1}`,
    artist: album.artist,
    album: album.title,
    originalYear: album.originalYear,
    releaseYear: album.releaseYear,
    publisher: album.publisher ?? null,
    rating: index < album.ratedTracks ? album.rating : null,
    loved: index < album.lovedTracks,
    loveState: index < album.lovedTracks ? "loved" : "neutral",
    tagSyncState: null,
    canUndoTagEdit: false,
    durationSeconds: Math.max(90, Math.round(album.durationSeconds / Math.max(1, album.totalTracks))),
    genre: album.genre,
    playCount: null,
  }));
}

export async function loadYearOverview(): Promise<YearOverview> {
  if (!isTauriRuntime()) return PREVIEW_OVERVIEW;
  return invoke<YearOverview>("year_overview");
}

export async function loadYearDetail(selection: YearSelection): Promise<YearDetail> {
  if (!isTauriRuntime()) return previewDetail(selection);
  return invoke<YearDetail>("year_detail", { selection });
}

export async function loadYearQueue(selection: YearSelection, limit = 100): Promise<Track[]> {
  if (!isTauriRuntime()) {
    const detail = previewDetail(selection);
    const tracks = detail.albums.flatMap(previewTracksForAlbum);
    return (tracks.length ? tracks : browserPreview.tracks).slice(0, limit);
  }
  return invoke<Track[]>("year_queue_tracks", { request: { selection, limit } });
}

export async function loadYearAlbumTracks(album: YearAlbum): Promise<Track[]> {
  if (!isTauriRuntime()) return previewTracksForAlbum(album);
  return (await loadAlbumDetail(album.id)).tracks;
}

export function yearSelectionLabel(selection: YearSelection): string {
  const basis = selection.basis === "original" ? "original" : "release";
  return selection.year === null ? `missing ${basis} year` : `${basis} ${selection.year}`;
}
