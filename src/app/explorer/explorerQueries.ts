import type { ExplorerFilters, ExplorerView } from "../../components/explorer/DeepExplorer";
import { exploreTracks, exploreAlbums, exploreArtists, type Track, type AlbumSummary, type Artist, type ExplorerCursor } from "../../library";
import { explorerSorts } from "../../viewPreferences";

export type ExplorerResult = {
  tracks: Track[];
  albums: AlbumSummary[];
  artists: Artist[];
  nextCursor: ExplorerCursor | null;
  totalCount: number;
};

export async function loadExplorerPage(
  view: ExplorerView,
  filters: ExplorerFilters,
  cursor?: ExplorerCursor,
  localOnly = true,
): Promise<ExplorerResult> {
  const shared = {
    pageSize: 50,
    cursor,
    search: filters.query.trim() || undefined,
    genre: filters.genre ?? undefined,
  };
  if (view === "tracks") {
    const page = await exploreTracks({
      ...shared,
      rating: typeof filters.rating === "number" ? filters.rating : undefined,
      unrated: filters.rating === "unrated" || undefined,
      loveState: filters.love === "all" ? undefined : filters.love,
      yearFrom: filters.yearFrom ?? undefined,
      yearTo: filters.yearTo ?? undefined,
      yearBasis: filters.yearBasis,
      missingYear: filters.yearMissing || undefined,
      artist: filters.artist ?? undefined,
      sort: explorerSorts.tracks.includes(filters.sort)
        ? filters.sort as "newest" | "oldest" | "titleAsc" | "titleDesc" | "artistAsc" | "artistDesc" | "albumAsc" | "albumDesc" | "yearAsc" | "yearDesc" | "releaseYearAsc" | "releaseYearDesc" | "ratingAsc" | "ratingDesc"
        : "newest",
    }, { localOnly });
    return { tracks: page.items, albums: [], artists: [], nextCursor: page.nextCursor, totalCount: page.totalCount };
  }
  if (view === "albums") {
    const page = await exploreAlbums({
      ...shared,
      rating: typeof filters.rating === "number" ? filters.rating : undefined,
      unrated: filters.rating === "unrated" || undefined,
      yearFrom: filters.yearFrom ?? undefined,
      yearTo: filters.yearTo ?? undefined,
      yearBasis: filters.yearBasis,
      missingYear: filters.yearMissing || undefined,
      artist: filters.artist ?? undefined,
      sort: explorerSorts.albums.includes(filters.sort)
        ? filters.sort as "newest" | "oldest" | "titleAsc" | "titleDesc" | "artistAsc" | "artistDesc" | "yearAsc" | "yearDesc" | "releaseYearAsc" | "releaseYearDesc" | "ratingAsc" | "ratingDesc"
        : "yearDesc",
    }, { localOnly });
    return { tracks: [], albums: page.items, artists: [], nextCursor: page.nextCursor, totalCount: page.totalCount };
  }
  const page = await exploreArtists({
    ...shared,
    sort: filters.sort === "trackCountDesc"
      ? "trackCountDesc"
      : filters.sort === "trackCountAsc"
        ? "trackCountAsc"
        : filters.sort === "artistDesc"
          ? "nameDesc"
          : "nameAsc",
  }, { localOnly });
  return { tracks: [], albums: [], artists: page.items, nextCursor: page.nextCursor, totalCount: page.totalCount };
}

export function explorerCountKey(view: ExplorerView, filters: ExplorerFilters): string {
  return JSON.stringify([
    view,
    filters.query.trim(),
    filters.rating,
    filters.love,
    filters.yearFrom,
    filters.yearTo,
    filters.yearBasis,
    filters.yearMissing,
    filters.genre,
    filters.artist,
  ]);
}

export function explorerRequestKey(view: ExplorerView, filters: ExplorerFilters, reloadToken: number): string {
  return JSON.stringify([view, filters, reloadToken]);
}
