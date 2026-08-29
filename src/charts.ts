import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, type Track } from "./library";

export type ChartKind = "singles" | "albums";
export type ChartSource = "officialUk" | "vgLista" | "tiISkuddet" | "norsktoppen" | "billboard" | "auroraScore";
export type ChartScope = "week" | "period";
export type ChartYearBasis = "year" | "releaseYear";

export interface ChartPeriod {
  fromYear: number;
  fromWeek: number;
  toYear: number;
  toWeek: number;
  label: string;
}

export interface ChartPageRequest {
  kind: ChartKind;
  source: ChartSource;
  scope: ChartScope;
  period: ChartPeriod;
  selectedYear: number;
  selectedWeek: number;
  yearBasis: ChartYearBasis;
  limit: number;
}

export interface ChartWeek {
  year: number;
  week: number;
  date: string | null;
}

export interface ChartEntry {
  position: number;
  sourcePosition: number;
  previousPosition: number | null;
  movement: number | null;
  peakPosition: number | null;
  appearances: number;
  weeksAtNumberOne: number;
  totalPoints: number;
  artist: string;
  title: string;
  artistKey: string;
  titleKey: string;
  matchedTrackId: string | null;
  matchedAlbumId: string | null;
  artworkAlbumId: string | null;
  rating: number | null;
  loved: boolean;
  albumScore: number | null;
}

export interface AlbumScoreEntry {
  id: string;
  title: string;
  artist: string;
  originalYear: number | null;
  releaseYear: number | null;
  score: number;
}

export interface ChartPage {
  request: ChartPageRequest;
  sourceLabel: string;
  chartTitle: string;
  annualOnly: boolean;
  chartDate: string | null;
  weeks: ChartWeek[];
  entries: ChartEntry[];
  totalEntries: number;
  albumScoreEntries: AlbumScoreEntry[];
}

export interface ChartSourceRank {
  source: ChartSource;
  label: string;
  bestRank: number | null;
  appearances: number;
  weeksAtNumberOne: number;
  annualOnly: boolean;
}

export interface ChartItemDetail {
  sourceRanks: ChartSourceRank[];
}

export interface CatalogChartRank {
  source: ChartSource;
  label: string;
  shortLabel: string;
  rank: number;
}

export interface CatalogChartRankings {
  tracks: Record<string, CatalogChartRank[]>;
  albums: Record<string, CatalogChartRank[]>;
}

export interface CatalogChartRankRequest {
  trackIds: string[];
  albumIds: string[];
}

export interface ChartItemDetailRequest {
  page: ChartPageRequest;
  artistKey: string;
  titleKey: string;
}

export const chartPresets: ReadonlyArray<ChartPeriod> = [
  { fromYear: 1985, fromWeek: 23, toYear: 1985, toWeek: 35, label: "Summer 1985" },
  { fromYear: 1972, fromWeek: 48, toYear: 1972, toWeek: 52, label: "Christmas 1972" },
  { fromYear: 2004, fromWeek: 5, toYear: 2004, toWeek: 9, label: "February 2004" },
  { fromYear: 1995, fromWeek: 7, toYear: 1995, toWeek: 13, label: "Week 7–13 · 1995" },
];

const previewTracks: Track[] = [
  { id: "preview-chart-crowd-track", trackKey: "preview:chart:crowd", albumId: "preview-chart-crowd", title: "You'll Never Walk Alone", artist: "The Crowd", album: "You'll Never Walk Alone", releaseYear: 1985, originalYear: 1985, rating: 4, loved: false, loveState: "neutral", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 161, genre: "Pop", playCount: null },
  { id: "preview-chart-kayleigh-track", trackKey: "preview:chart:kayleigh", albumId: "preview-chart-kayleigh", title: "Kayleigh", artist: "Marillion", album: "Misplaced Childhood", releaseYear: 1985, originalYear: 1985, rating: 4, loved: true, loveState: "loved", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 243, genre: "Progressive Rock", playCount: null },
  { id: "preview-chart-19-track", trackKey: "preview:chart:19", albumId: "preview-chart-19", title: "19", artist: "Paul Hardcastle", album: "Paul Hardcastle", releaseYear: 1985, originalYear: 1985, rating: 4.5, loved: false, loveState: "neutral", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 219, genre: "Electronic", playCount: null },
  { id: "preview-chart-suddenly-track", trackKey: "preview:chart:suddenly", albumId: "preview-chart-suddenly", title: "Suddenly", artist: "Billy Ocean", album: "Suddenly", releaseYear: 1984, originalYear: 1984, rating: 4, loved: false, loveState: "neutral", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 233, genre: "Soul", playCount: null },
  { id: "preview-chart-obsession-track", trackKey: "preview:chart:obsession", albumId: "preview-chart-obsession", title: "Obsession", artist: "Animotion", album: "Animotion", releaseYear: 1984, originalYear: 1984, rating: 4.5, loved: true, loveState: "loved", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 240, genre: "Synth-pop", playCount: null },
  { id: "preview-chart-view-track", trackKey: "preview:chart:view", albumId: "preview-chart-view", title: "A View to a Kill", artist: "Duran Duran", album: "A View to a Kill", releaseYear: 1985, originalYear: 1985, rating: 5, loved: true, loveState: "loved", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 217, genre: "Pop Rock", playCount: null },
  { id: "preview-chart-fields-track", trackKey: "preview:chart:fields", albumId: "preview-chart-fields", title: "Out in the Fields", artist: "Gary Moore and Phil Lynott", album: "Run for Cover", releaseYear: 1985, originalYear: 1985, rating: 4.5, loved: false, loveState: "neutral", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 258, genre: "Hard Rock", playCount: null },
  { id: "preview-chart-word-track", trackKey: "preview:chart:word", albumId: "preview-chart-word", title: "The Word Girl", artist: "Scritti Politti", album: "Cupid & Psyche 85", releaseYear: 1985, originalYear: 1985, rating: 4, loved: true, loveState: "loved", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 255, genre: "New Wave", playCount: null },
  { id: "preview-chart-crazy-track", trackKey: "preview:chart:crazy", albumId: "preview-chart-crazy", title: "Crazy for You", artist: "Madonna", album: "Like a Virgin", releaseYear: 1984, originalYear: 1984, rating: 5, loved: true, loveState: "loved", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 224, genre: "Pop", playCount: null },
  { id: "preview-chart-history-track", trackKey: "preview:chart:history", albumId: "preview-chart-history", title: "History", artist: "Mai Tai", album: "Mai Tai", releaseYear: 1985, originalYear: 1985, rating: 4, loved: false, loveState: "neutral", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 212, genre: "Dance", playCount: null },
];

const previewEntries: ChartEntry[] = previewTracks.map((track, index) => ({
  position: index + 1,
  sourcePosition: index + 1,
  previousPosition: [1, 3, 2, 4, 7, 8, 5, 8, 11, 9][index],
  movement: [0, 1, -1, 0, 2, 2, -2, 0, 2, -1][index],
  peakPosition: [1, 1, 1, 3, 5, 6, 5, 8, 9, 9][index],
  appearances: [4, 10, 7, 6, 7, 5, 6, 5, 4, 4][index],
  weeksAtNumberOne: index === 1 ? 2 : 0,
  totalPoints: [1452, 1381, 1214, 1143, 987, 902, 821, 742, 698, 601][index],
  artist: track.artist,
  title: track.title,
  artistKey: track.artist.toLocaleLowerCase(),
  titleKey: track.title.toLocaleLowerCase(),
  matchedTrackId: track.id,
  matchedAlbumId: track.albumId,
  artworkAlbumId: track.albumId,
  rating: track.rating,
  loved: track.loved,
  albumScore: null,
}));

const previewScores: AlbumScoreEntry[] = [
  { id: "preview-score-rocky", title: "Rocky IV", artist: "Various Artists", originalYear: 1985, releaseYear: 2006, score: 815.0 },
  { id: "preview-score-miami", title: "Miami Vice", artist: "Various Artists", originalYear: 1985, releaseYear: 1985, score: 615.3 },
  { id: "preview-score-back-future", title: "Back to the Future", artist: "Various Artists", originalYear: 1985, releaseYear: 1985, score: 613.8 },
  { id: "preview-score-american-flyers", title: "American Flyers", artist: "Various Artists", originalYear: 1985, releaseYear: 1985, score: 514.8 },
  { id: "preview-score-magnum", title: "On a Storyteller's Night", artist: "Magnum", originalYear: 1985, releaseYear: 1985, score: 416.4 },
  { id: "preview-score-kind-blue", title: "Kind of Blue", artist: "Miles Davis", originalYear: 1959, releaseYear: 1985, score: 390.2 },
];

function previewWeeks(period: ChartPeriod): ChartWeek[] {
  if (period.fromYear === period.toYear) {
    return Array.from({ length: period.toWeek - period.fromWeek + 1 }, (_, index) => ({
      year: period.fromYear,
      week: period.fromWeek + index,
      date: period.fromYear === 1985 && period.fromWeek + index === 23 ? "1985-06-09" : null,
    }));
  }
  return [{ year: period.fromYear, week: period.fromWeek, date: null }, { year: period.toYear, week: period.toWeek, date: null }];
}

function scoreEntries(scores: readonly AlbumScoreEntry[]): ChartEntry[] {
  return scores.map((album, index) => ({
    position: index + 1,
    sourcePosition: index + 1,
    previousPosition: null,
    movement: null,
    peakPosition: index + 1,
    appearances: 1,
    weeksAtNumberOne: index === 0 ? 1 : 0,
    totalPoints: Math.round(album.score),
    artist: album.artist,
    title: album.title,
    artistKey: album.artist.toLocaleLowerCase(),
    titleKey: album.title.toLocaleLowerCase(),
    matchedTrackId: null,
    matchedAlbumId: album.id,
    artworkAlbumId: album.id,
    rating: null,
    loved: index < 2,
    albumScore: album.score,
  }));
}

function browserChartPage(request: ChartPageRequest): ChartPage {
  const annualOnly = request.source === "billboard" || request.source === "auroraScore";
  const sourceLabel: Record<ChartSource, string> = {
    officialUk: "Official UK",
    vgLista: "VG Lista",
    tiISkuddet: "Ti i Skuddet",
    norsktoppen: "Norsktoppen",
    billboard: "Billboard",
    auroraScore: "Aurora Score",
  };
  const scoreYear = (album: AlbumScoreEntry) => request.yearBasis === "year" ? album.originalYear : album.releaseYear;
  const scores = previewScores
    .filter((album) => {
      const year = scoreYear(album);
      return year !== null && year >= request.period.fromYear && year <= request.period.toYear;
    })
    .sort((left, right) => right.score - left.score);
  const useScores = request.kind === "albums";
  const entries = (useScores ? scoreEntries(scores) : previewEntries).map((entry, index) => ({
    ...entry,
    position: index + 1,
    sourcePosition: index + 1,
    totalPoints: request.scope === "period" ? entry.totalPoints : Math.max(1, 101 - index),
  }));
  const effectiveRequest = { ...request, scope: annualOnly ? "period" as const : request.scope };
  return {
    request: effectiveRequest,
    sourceLabel: sourceLabel[request.source],
    chartTitle: request.source === "auroraScore"
      ? `Aurora Album Score · ${request.period.label}`
      : `${sourceLabel[request.source]} ${request.kind === "singles" ? "Singles" : "Albums"}${effectiveRequest.scope === "period" ? ` · ${request.period.label}` : " Chart"}`,
    annualOnly,
    chartDate: effectiveRequest.scope === "week" ? "1985-06-09" : null,
    weeks: annualOnly ? [] : previewWeeks(request.period),
    entries,
    totalEntries: entries.length,
    albumScoreEntries: scores.slice(0, 5),
  };
}

export async function loadChartPage(request: ChartPageRequest): Promise<ChartPage> {
  if (!isTauriRuntime()) return browserChartPage(request);
  return invoke<ChartPage>("chart_page", { request });
}

export async function loadChartItemDetail(request: ChartItemDetailRequest): Promise<ChartItemDetail> {
  if (!isTauriRuntime()) {
    const entry = previewEntries.find((candidate) => candidate.artistKey === request.artistKey && candidate.titleKey === request.titleKey);
    const base = entry?.position ?? 4;
    return { sourceRanks: [
      { source: "officialUk", label: "Official UK", bestRank: base, appearances: entry?.appearances ?? 1, weeksAtNumberOne: entry?.weeksAtNumberOne ?? 0, annualOnly: false },
      { source: "vgLista", label: "VG Lista", bestRank: Math.max(1, base - 1), appearances: 7, weeksAtNumberOne: base <= 2 ? 1 : 0, annualOnly: false },
      { source: "tiISkuddet", label: "Ti i Skuddet", bestRank: Math.min(10, base + 1), appearances: 5, weeksAtNumberOne: 0, annualOnly: false },
      { source: "norsktoppen", label: "Norsktoppen", bestRank: base <= 4 ? base + 2 : null, appearances: base <= 4 ? 3 : 0, weeksAtNumberOne: 0, annualOnly: false },
      { source: "billboard", label: "Billboard", bestRank: base === 2 ? 74 : null, appearances: base === 2 ? 1 : 0, weeksAtNumberOne: 0, annualOnly: true },
    ] };
  }
  return invoke<ChartItemDetail>("chart_item_detail", { request });
}

export async function loadCatalogChartRankings(request: CatalogChartRankRequest): Promise<CatalogChartRankings> {
  if (!isTauriRuntime()) {
    const tracks = Object.fromEntries(request.trackIds.flatMap((id, index) => {
      if (index % 3 !== 0) return [];
      const ranks = [
        { source: "billboard", label: "Billboard", shortLabel: "BB", rank: 4 + index },
        { source: "officialUk", label: "Official UK", shortLabel: "UK", rank: 14 + index },
        { source: "vgLista", label: "VG Lista", shortLabel: "VG", rank: 1 + index },
        { source: "tiISkuddet", label: "Ti i Skuddet", shortLabel: "TI", rank: 10 + index },
        { source: "norsktoppen", label: "Norsktoppen", shortLabel: "NT", rank: 15 + index },
      ] satisfies CatalogChartRank[];
      return [[id, index === 0 ? ranks : ranks.slice(0, 1)]];
    }));
    const albums = Object.fromEntries(request.albumIds.flatMap((id, index) => {
      if (index % 2 !== 0) return [];
      const ranks = [
        { source: "billboard", label: "Billboard", shortLabel: "US", rank: 14 + index },
        { source: "officialUk", label: "Official UK", shortLabel: "UK", rank: 4 + index },
        { source: "vgLista", label: "VG Lista", shortLabel: "NO", rank: 1 + index },
      ] satisfies CatalogChartRank[];
      return [[id, index === 0 ? ranks : ranks.slice(0, 1)]];
    }));
    return { tracks, albums };
  }
  return invoke<CatalogChartRankings>("catalog_chart_rankings", { request });
}

export async function loadChartEntryTrack(trackId: string): Promise<Track> {
  if (!isTauriRuntime()) {
    const track = previewTracks.find((candidate) => candidate.id === trackId);
    if (!track) throw new Error("This chart entry is not matched to a preview track.");
    return track;
  }
  return invoke<Track>("chart_entry_track", { trackId });
}

export async function loadChartQueue(request: ChartPageRequest): Promise<Track[]> {
  if (!isTauriRuntime()) return request.kind === "singles" ? previewTracks : previewTracks.slice(0, 5);
  return invoke<Track[]>("chart_queue_tracks", { request });
}
