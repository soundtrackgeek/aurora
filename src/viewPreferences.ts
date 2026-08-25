import type {
  ExplorerFilters,
  ExplorerRatingFilter,
  ExplorerSort,
  ExplorerView,
} from "./components/explorer/DeepExplorer";
import type { SidebarDestination } from "./components/navigation/SidebarNavigation";

export type InspectorView = "track" | "album" | "artist" | "tags";
export type TagSelectionKind = "track" | "album";

export type ViewPreferences = {
  activeNav: SidebarDestination;
  explorerView: ExplorerView;
  explorerFilters: ExplorerFilters;
  inspectorView: InspectorView;
  tagSelectionKind: TagSelectionKind;
  selectedAlbumId: string | null;
};

type StoredViewPreferencesV1 = ViewPreferences & {
  schemaVersion: 1;
};

type ViewStorage = Pick<Storage, "getItem" | "setItem">;

const STORAGE_KEY = "aurora:view-preferences:v1";

export const defaultExplorerFilters: ExplorerFilters = {
  query: "",
  rating: "all",
  love: "all",
  yearFrom: null,
  yearTo: null,
  yearBasis: "original",
  yearMissing: false,
  genre: null,
  artist: null,
  sort: "newest",
};

export const explorerSorts: Record<ExplorerView, readonly ExplorerSort[]> = {
  tracks: ["newest", "oldest", "titleAsc", "titleDesc", "artistAsc", "artistDesc", "albumAsc", "albumDesc", "yearAsc", "yearDesc", "releaseYearAsc", "releaseYearDesc", "ratingAsc", "ratingDesc"],
  albums: ["newest", "oldest", "yearAsc", "yearDesc", "releaseYearAsc", "releaseYearDesc", "titleAsc", "titleDesc", "artistAsc", "artistDesc", "ratingAsc", "ratingDesc"],
  artists: ["artistAsc", "artistDesc", "trackCountAsc", "trackCountDesc"],
};

export const defaultExplorerSort: Record<ExplorerView, ExplorerSort> = {
  tracks: "newest",
  albums: "yearDesc",
  artists: "artistAsc",
};

export const defaultViewPreferences: ViewPreferences = {
  activeNav: "Universe",
  explorerView: "tracks",
  explorerFilters: defaultExplorerFilters,
  inspectorView: "track",
  tagSelectionKind: "track",
  selectedAlbumId: null,
};

const destinations = new Set<SidebarDestination>([
  "Universe",
  "Inbox",
  "Observatory",
  "Songs",
  "Albums",
  "Artists",
  "Publishers",
  "Genres",
  "Years",
  "Ratings",
  "Tags",
  "Charts",
  "History",
]);
const explorerViews = new Set<ExplorerView>(["tracks", "albums", "artists"]);
const ratingFilters = new Set<ExplorerRatingFilter>(["all", "unrated", 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4, 4.5, 5]);
const loveFilters = new Set<ExplorerFilters["love"]>(["all", "neutral", "loved", "banned"]);
const inspectorViews = new Set<InspectorView>(["track", "album", "artist", "tags"]);
const tagSelectionKinds = new Set<TagSelectionKind>(["track", "album"]);

function createDefaultViewPreferences(): ViewPreferences {
  return {
    ...defaultViewPreferences,
    explorerFilters: { ...defaultExplorerFilters },
  };
}

function browserStorage(): ViewStorage | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === "string";
}

function isNullableYear(value: unknown): value is number | null {
  return value === null || (typeof value === "number" && Number.isInteger(value));
}

function validFilters(value: unknown, view: ExplorerView): value is ExplorerFilters {
  if (!value || typeof value !== "object") return false;
  const filters = value as Partial<ExplorerFilters>;
  return typeof filters.query === "string"
    && ratingFilters.has(filters.rating as ExplorerRatingFilter)
    && loveFilters.has(filters.love as ExplorerFilters["love"])
    && isNullableYear(filters.yearFrom)
    && isNullableYear(filters.yearTo)
    && (filters.yearBasis === "original" || filters.yearBasis === "release")
    && typeof filters.yearMissing === "boolean"
    && isNullableString(filters.genre)
    && isNullableString(filters.artist)
    && explorerSorts[view].includes(filters.sort as ExplorerSort);
}

export function loadViewPreferences(storage: ViewStorage | null = browserStorage()): ViewPreferences {
  const fallback = createDefaultViewPreferences();
  if (!storage) return fallback;
  try {
    const raw = storage.getItem(STORAGE_KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as Partial<StoredViewPreferencesV1>;
    if (
      parsed.schemaVersion !== 1
      || !destinations.has(parsed.activeNav as SidebarDestination)
      || !explorerViews.has(parsed.explorerView as ExplorerView)
      || !validFilters(parsed.explorerFilters, parsed.explorerView as ExplorerView)
      || !inspectorViews.has(parsed.inspectorView as InspectorView)
      || !tagSelectionKinds.has(parsed.tagSelectionKind as TagSelectionKind)
      || !isNullableString(parsed.selectedAlbumId)
    ) return fallback;

    return {
      activeNav: parsed.activeNav as SidebarDestination,
      explorerView: parsed.explorerView as ExplorerView,
      explorerFilters: { ...parsed.explorerFilters },
      inspectorView: parsed.inspectorView as InspectorView,
      tagSelectionKind: parsed.tagSelectionKind as TagSelectionKind,
      selectedAlbumId: parsed.selectedAlbumId,
    };
  } catch {
    return fallback;
  }
}

export function saveViewPreferences(
  preferences: ViewPreferences,
  storage: ViewStorage | null = browserStorage(),
): boolean {
  if (!storage) return false;
  const stored: StoredViewPreferencesV1 = { schemaVersion: 1, ...preferences };
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(stored));
    return true;
  } catch {
    return false;
  }
}
