import {
  Activity,
  Album,
  AudioLines,
  BadgeCheck,
  ChevronRight,
  CircleUserRound,
  Clock3,
  Disc3,
  Download,
  FlaskConical,
  FolderPlus,
  Gauge,
  Heart,
  Music2,
  PanelLeft,
  PanelLeftClose,
  PanelLeftOpen,
  PanelRightClose,
  PanelRightOpen,
  Play,
  RefreshCw,
  Search,
  Settings,
  UsersRound,
  X,
} from "lucide-react";
import { lazy, Suspense, type FormEvent, useCallback, useEffect, useRef, useState } from "react";
import "./App.css";
import { Artwork } from "./components/Artwork";
import {
  ChartInspector,
  ChartStudio,
  type ChartSelectionContext,
} from "./components/charts/ChartStudio";
import { LaptopModeButton } from "./components/LaptopModeButton";
import {
  DeepExplorer,
  type ExplorerAlbum,
  type ExplorerFilters,
  type ExplorerLoadState,
  type ExplorerView,
} from "./components/explorer/DeepExplorer";
import { resolveExplorerAlbumInspectorContext } from "./components/explorer/inspectorContext";
import { ArtistWorld, type ArtistWorldState } from "./components/musicbrainz/ArtistWorld";
import { Observatory, type ObservatoryLoadState } from "./components/curation/Observatory";
import {
  ListeningHistory,
  type HistoryDateRange,
  type HistoryLoadState,
} from "./components/history/ListeningHistory";
import { GenreAtlas, type GenreAtlasLoadState } from "./components/genres/GenreAtlas";
import {
  YearAlbumInspector,
  YearsExplorer,
  type YearsLoadState,
} from "./components/library/YearsExplorer";
import {
  RatingAlbumInspector,
  RatingsStudio,
  type RatingsLoadState,
} from "./components/ratings/RatingsStudio";
import {
  PublisherAlbumInspector,
  PublisherSignalTimeline,
  type PublisherLoadState,
} from "./components/publishers/PublisherSignalTimeline";
import {
  SidebarNavigation,
  type SidebarDestination,
} from "./components/navigation/SidebarNavigation";
import { PlayerBar } from "./components/PlayerBar";
import { QueuePanel } from "./components/QueuePanel";
import { SettingsDialog, type SettingsTab } from "./components/SettingsDialog";
import { TagEditor } from "./components/TagEditor";
import {
  exploreAlbums,
  exploreArtists,
  exploreTracks,
  displayTrackArtist,
  formatCount,
  formatDuration,
  catalogRefreshIsConsistent,
  loadAlbumDetail,
  loadArtistDetail,
  loadCatalogRevision,
  loadLibrarySnapshot,
  applyEditableTrackTagProjection,
  applyTrackTagProjection,
  type AlbumSummary,
  type Artist,
  type ArtistDetail,
  type ExplorerCursor,
  type LibrarySnapshot,
  type Track,
} from "./library";
import {
  exportMusicBrainzCuration,
  loadArtistIntelligence,
  loadArtistReviewPage,
  undoMusicBrainzCuration,
  updateArtistIdentityDecision,
  updateReleaseGroupDecision,
  type ArtistDecisionRequest,
  type ArtistIntelligence,
  type ArtistReviewFilter,
  type ArtistReviewItem,
  type ReleaseDecisionRequest,
} from "./musicbrainz";
import { usePlayback } from "./playback";
import {
  loadHistoryPage,
  loadTrackHistoryInsight,
  saveHistoryPlayThreshold,
  type HistoryOutcomeFilter,
  type HistoryPage,
  type TrackHistoryInsight,
} from "./history";
import {
  loadGenreDetail,
  loadGenreIndex,
  loadGenreQueue,
  saveGenreRadioSession,
  type GenreDetail,
  type GenreQueueMode,
  type GenreRadioSession,
  type GenreSummary,
} from "./genres";
import {
  loadLaptopModeStatus,
  updateLaptopMode,
  type LaptopModeStatus,
} from "./laptopMode";
import {
  loadLayoutPreferences,
  nextLeftSidebarMode,
  saveLayoutPreferences,
} from "./layoutPreferences";
import {
  defaultExplorerFilters,
  defaultExplorerSort,
  explorerSorts,
  loadViewPreferences,
  saveViewPreferences,
} from "./viewPreferences";
import {
  effectiveDisplayPreferences,
  loadDisplayPreferences,
  saveDisplayPreferences,
  type DisplayViewKey,
} from "./displayPreferences";
import {
  advanceCatalogProjectionToken,
  advanceCatalogTrackProjectionTokens,
  reconcilePendingTags,
  tagValuesForTrack,
  trackWithReconciledTags,
  trackWithTagValues,
  updateTrackTags,
  type CatalogSync,
  type TagReconciliationChange,
  type TagValues,
} from "./tags";
import { useAuroraUpdater } from "./updater";
import {
  loadRatingAlbumPage,
  loadRatingAlbumQueue,
  loadRatingAlbumTracks,
  loadRatingCollection,
  loadRatingsOverview,
  type CompletionKind,
  type RatingAlbum,
  type RatingAlbumPage,
  type RatingMode,
  type RatingsOverview,
} from "./ratings";

import {
  listenForGlobalShortcutResults,
  loadGlobalShortcutSettings,
  defaultShortcutBindings,
  updateGlobalShortcutSettings,
  type GlobalShortcutSettingsRequest,
  type GlobalShortcutStatus,
  type GlobalShortcutResult,
} from "./shortcuts";
import {
  loadAudioSettings,
  updateAudioSettings,
  type AudioSettingsRequest,
  type AudioSettingsStatus,
} from "./audio";
import {
  loadChartEntryTrack,
} from "./charts";
import {
  loadYearAlbumTracks,
  loadYearDetail,
  loadYearOverview,
  loadYearQueue,
  type YearAlbum,
  type YearDetail,
  type YearOverview,
  type YearSelection,
} from "./years";
import {
  loadPublisherAlbumTracks,
  loadPublisherDetail,
  loadPublisherOverview,
  loadPublisherQueue,
  type PublisherAlbum,
  type PublisherDetail,
  type PublisherOverview,
  type PublisherSummary,
} from "./publishers";

const AddFolderDialog = lazy(async () => {
  const module = await import("./components/AddFolderDialog");
  return { default: module.AddFolderDialog };
});

const displayViewByDestination: Record<SidebarDestination, DisplayViewKey> = {
  Universe: "universe",
  Observatory: "observatory",
  Songs: "songs",
  Albums: "albums",
  Artists: "artists",
  Publishers: "publishers",
  Genres: "genres",
  Years: "years",
  Ratings: "ratings",
  Tags: "tags",
  Charts: "charts",
  History: "history",
};

const trackSearchHelp = "Fields: artist (Display Artist), aartist (Album Artist display), album, genre, year (Year), ryear (Release Year), publisher, and title. Years accept inclusive ranges such as year:1985..1987, year:1985.., and year:..1987; the same syntax works for ryear. Use commas or uppercase AND between groups; uppercase OR inherits the preceding field; NOT or a leading - excludes. Quote a complete value for an exact match. genre:scores includes film, TV, animation, anime, and game scores.";
const catalogSyncRetryIntervalMs = 5_000;

function catalogSyncNeedsRetry(sync: CatalogSync | null): boolean {
  return Boolean(sync && (sync.status === "pending" || sync.pendingFolderCount > 0));
}

function catalogSyncMessage(sync: CatalogSync): string {
  if (sync.status === "synced" && sync.pendingFolderCount > 0) {
    return `Music Library updated this edit · ${formatCount(sync.pendingFolderCount)} other ${sync.pendingFolderCount === 1 ? "folder is" : "folders are"} pending; retrying automatically`;
  }
  if (sync.status === "pending") {
    return `MP3 changes are saved · ${sync.message?.trim() || "Music Library update pending; retrying automatically."}`;
  }
  return sync.message?.trim() || "Music Library updated.";
}

function genreSummaryWithTrackChange(
  summary: GenreSummary,
  before: Track,
  after: Track,
): GenreSummary {
  if (summary.name !== before.genre || before.genre !== after.genre) return summary;
  const ratedTracks = Math.max(
    0,
    summary.ratedTracks + Number(after.rating !== null) - Number(before.rating !== null),
  );
  const ratingSum = (summary.averageRating ?? 0) * summary.ratedTracks
    - (before.rating ?? 0)
    + (after.rating ?? 0);
  return {
    ...summary,
    ratedTracks,
    averageRating: ratedTracks > 0 ? Math.min(5, Math.max(0, ratingSum / ratedTracks)) : null,
    lovedTracks: Math.max(0, summary.lovedTracks + Number(after.loved) - Number(before.loved)),
  };
}

type ExplorerResult = {
  tracks: Track[];
  albums: AlbumSummary[];
  artists: Artist[];
  nextCursor: ExplorerCursor | null;
  totalCount: number;
};

async function loadExplorerPage(
  view: ExplorerView,
  filters: ExplorerFilters,
  cursor?: ExplorerCursor,
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
    });
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
        ? filters.sort as "titleAsc" | "titleDesc" | "artistAsc" | "artistDesc" | "yearAsc" | "yearDesc" | "releaseYearAsc" | "releaseYearDesc" | "ratingAsc" | "ratingDesc"
        : "yearDesc",
    });
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
  });
  return { tracks: [], albums: [], artists: page.items, nextCursor: page.nextCursor, totalCount: page.totalCount };
}

function explorerCountKey(view: ExplorerView, filters: ExplorerFilters): string {
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

function historyDateLabel(timestamp: number | null): string {
  if (timestamp === null) return "Never";
  return new Intl.DateTimeFormat(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp));
}

function EmptyInspector() {
  return (
    <div className="inspector-empty">
      <Disc3 aria-hidden="true" />
      <h2>Select a track</h2>
      <p>Select a song, then press play or double-click its library row.</p>
    </div>
  );
}

function Universe({ artists, activeArtist, onSelect }: { artists: Artist[]; activeArtist: string | null; onSelect: (artist: Artist) => void }) {
  const visibleArtists = artists.slice(0, 8);
  return (
    <section className="universe" aria-labelledby="universe-title">
      <div className="universe__heading">
        <p className="eyebrow">Your listening gravity</p>
        <h1 id="universe-title">Welcome back, Jørn.</h1>
        <p>Choose a world to focus the library.</p>
      </div>
      <div className="universe__quote">
        <p>“Music is the shorthand of emotion.”</p>
        <span>— Leo Tolstoy</span>
      </div>
      <div className="solar-system" aria-label="Top artists in your music universe">
        <span className="sun" aria-hidden="true" />
        {[0, 1, 2, 3].map((orbit) => <span className={`orbit orbit--${orbit + 1}`} key={orbit} aria-hidden="true" />)}
        {visibleArtists.map((artist, index) => (
          <button
            type="button"
            className={`planet planet--${index + 1}${activeArtist === artist.name ? " is-active" : ""}`}
            key={artist.id}
            onClick={() => onSelect(artist)}
            aria-label={`Explore ${artist.name}, ${formatCount(artist.trackCount)} tracks`}
            aria-pressed={activeArtist === artist.name}
          >
            <span className="planet__dot" />
            <span className="planet__label">{artist.name}</span>
          </button>
        ))}
      </div>
    </section>
  );
}

function UpdateDialog({ version, phase, progress, message, onInstall, onDismiss }: {
  version: string | null;
  phase: string;
  progress: number | null;
  message: string | null;
  onInstall: () => void;
  onDismiss: () => void;
}) {
  const isWorking = phase === "downloading" || phase === "installing";
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="update-dialog" role="dialog" aria-modal="true" aria-labelledby="update-title">
        <div className="update-dialog__icon"><Download aria-hidden="true" /></div>
        <div>
          <p className="eyebrow">Aurora update</p>
          <h2 id="update-title">Version {version ?? "unknown"} is ready</h2>
          <p>{message || "Install the latest Aurora build now. The app will close, update in place, and restart."}</p>
        </div>
        {isWorking && (
          <div className="update-progress" aria-live="polite">
            <div className="update-progress__track"><span style={{ width: `${progress ?? 12}%` }} /></div>
            <span>{phase === "installing" ? "Installing…" : progress === null ? "Downloading…" : `Downloading ${progress}%`}</span>
          </div>
        )}
        {phase === "error" && <p className="update-error" role="alert">{message}</p>}
        <div className="update-dialog__actions">
          <button type="button" className="button button--quiet" onClick={onDismiss} disabled={isWorking}>Later</button>
          <button type="button" className="button button--primary" onClick={onInstall} disabled={isWorking}>
            {isWorking ? "Updating…" : "Install and restart"}
          </button>
        </div>
      </section>
    </div>
  );
}

function App() {
  const [initialViewPreferences] = useState(loadViewPreferences);
  const [snapshot, setSnapshot] = useState<LibrarySnapshot | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selectedTrack, setSelectedTrack] = useState<Track | null>(null);
  const [trackHistory, setTrackHistory] = useState<{ trackKey: string; value: TrackHistoryInsight } | null>(null);
  const [inspectorView, setInspectorView] = useState(initialViewPreferences.inspectorView);
  const [tagSelectionKind, setTagSelectionKind] = useState(initialViewPreferences.tagSelectionKind);
  const [inspectorArtistName, setInspectorArtistName] = useState<string | null>(null);
  const [artistDetail, setArtistDetail] = useState<ArtistDetail | null>(null);
  const [artistIntelligence, setArtistIntelligence] = useState<ArtistIntelligence | null>(null);
  const [artistWorldState, setArtistWorldState] = useState<ArtistWorldState>("loading");
  const [artistWorldError, setArtistWorldError] = useState<string | null>(null);
  const [curationError, setCurationError] = useState<string | null>(null);
  const [curationActionBusy, setCurationActionBusy] = useState<string | null>(null);
  const [curationMessage, setCurationMessage] = useState<string | null>(null);
  const [reviewItems, setReviewItems] = useState<ArtistReviewItem[]>([]);
  const [reviewCursor, setReviewCursor] = useState<string | null>(null);
  const [reviewFilter, setReviewFilter] = useState<ArtistReviewFilter>("needsReview");
  const [reviewSearch, setReviewSearch] = useState("");
  const [reviewLoadState, setReviewLoadState] = useState<ObservatoryLoadState>("loading");
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [reviewLoadingMore, setReviewLoadingMore] = useState(false);
  const [reviewReloadToken, setReviewReloadToken] = useState(0);
  const [activeNav, setActiveNav] = useState<SidebarDestination>(initialViewPreferences.activeNav);
  const [layoutPreferences, setLayoutPreferences] = useState(loadLayoutPreferences);
  const [displayPreferences, setDisplayPreferences] = useState(loadDisplayPreferences);
  const [reloadToken, setReloadToken] = useState(0);
  const [explorerView, setExplorerView] = useState<ExplorerView>(initialViewPreferences.explorerView);
  const [explorerFilters, setExplorerFilters] = useState<ExplorerFilters>(initialViewPreferences.explorerFilters);
  const [explorerTracks, setExplorerTracks] = useState<Track[]>([]);
  const [explorerAlbums, setExplorerAlbums] = useState<AlbumSummary[]>([]);
  const [explorerArtists, setExplorerArtists] = useState<Artist[]>([]);
  const [explorerCursor, setExplorerCursor] = useState<ExplorerCursor | null>(null);
  const [explorerCount, setExplorerCount] = useState<{ key: string; total: number } | null>(null);
  const [explorerLoadState, setExplorerLoadState] = useState<ExplorerLoadState>("loading");
  const [explorerError, setExplorerError] = useState<string | null>(null);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [explorerReloadToken, setExplorerReloadToken] = useState(0);
  const [selectedAlbumId, setSelectedAlbumId] = useState<string | null>(initialViewPreferences.selectedAlbumId);
  const [albumTracks, setAlbumTracks] = useState<Track[]>([]);
  const [albumTracksTruncated, setAlbumTracksTruncated] = useState(false);
  const [albumDetailState, setAlbumDetailState] = useState<ExplorerLoadState>("ready");
  const [selectedArtistId, setSelectedArtistId] = useState<string | null>(null);
  const [syncMessage, setSyncMessage] = useState<string | null>(null);
  const [catalogSyncNotice, setCatalogSyncNotice] = useState<CatalogSync | null>(null);
  const [reconciliationHasMore, setReconciliationHasMore] = useState(false);
  const [queueOpen, setQueueOpen] = useState(false);
  const [addFolderOpen, setAddFolderOpen] = useState(false);
  const [inlineSavingKeys, setInlineSavingKeys] = useState<Set<string>>(() => new Set());
  const [inlineTagRevisions, setInlineTagRevisions] = useState<Record<string, number>>({});
  const [laptopModeStatus, setLaptopModeStatus] = useState<LaptopModeStatus | null>(null);
  const [laptopModeBusy, setLaptopModeBusy] = useState(false);
  const [laptopModeError, setLaptopModeError] = useState<string | null>(null);
  const [historyPage, setHistoryPage] = useState<HistoryPage | null>(null);
  const [historyLoadState, setHistoryLoadState] = useState<HistoryLoadState>("loading");
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [historySearch, setHistorySearch] = useState("");
  const [historyOutcome, setHistoryOutcome] = useState<HistoryOutcomeFilter>("all");
  const [historyDeviceId, setHistoryDeviceId] = useState<string | null>(null);
  const [historyDateRange, setHistoryDateRange] = useState<HistoryDateRange>("all");
  const [historyLoadingMore, setHistoryLoadingMore] = useState(false);
  const [historySavingThreshold, setHistorySavingThreshold] = useState(false);
  const [historyThresholdMessage, setHistoryThresholdMessage] = useState<string | null>(null);
  const [historyReloadToken, setHistoryReloadToken] = useState(0);
  const [genreAtlasGenres, setGenreAtlasGenres] = useState<GenreSummary[]>([]);
  const [selectedGenre, setSelectedGenre] = useState<string | null>(null);
  const [genreDetail, setGenreDetail] = useState<GenreDetail | null>(null);
  const [genreSearch, setGenreSearch] = useState("");
  const [genreIndexState, setGenreIndexState] = useState<GenreAtlasLoadState>("loading");
  const [genreDetailState, setGenreDetailState] = useState<GenreAtlasLoadState>("loading");
  const [genreIndexError, setGenreIndexError] = useState<string | null>(null);
  const [genreDetailError, setGenreDetailError] = useState<string | null>(null);
  const [genreIndexReloadToken, setGenreIndexReloadToken] = useState(0);
  const [genreDetailReloadToken, setGenreDetailReloadToken] = useState(0);
  const [genreQueueBusy, setGenreQueueBusy] = useState<GenreQueueMode | null>(null);
  const [genreQueueMessage, setGenreQueueMessage] = useState<string | null>(null);
  const [genreRadioSession, setGenreRadioSession] = useState<GenreRadioSession | null>(null);
  const [publisherOverview, setPublisherOverview] = useState<PublisherOverview | null>(null);
  const [publisherDetail, setPublisherDetail] = useState<PublisherDetail | null>(null);
  const [publisherLoadState, setPublisherLoadState] = useState<PublisherLoadState>("loading");
  const [publisherDetailState, setPublisherDetailState] = useState<PublisherLoadState>("loading");
  const [publisherError, setPublisherError] = useState<string | null>(null);
  const [publisherDetailError, setPublisherDetailError] = useState<string | null>(null);
  const [publisherSearch, setPublisherSearch] = useState("");
  const [publisherReloadToken, setPublisherReloadToken] = useState(0);
  const [publisherQueueBusy, setPublisherQueueBusy] = useState(false);
  const [publisherQueueMessage, setPublisherQueueMessage] = useState<string | null>(null);
  const [selectedPublisherAlbum, setSelectedPublisherAlbum] = useState<PublisherAlbum | null>(null);
  const [publisherAlbumTracks, setPublisherAlbumTracks] = useState<Track[]>([]);
  const [publisherAlbumBusy, setPublisherAlbumBusy] = useState(false);
  const [yearOverview, setYearOverview] = useState<YearOverview | null>(null);
  const [yearDetail, setYearDetail] = useState<YearDetail | null>(null);
  const [yearLoadState, setYearLoadState] = useState<YearsLoadState>("loading");
  const [yearDetailState, setYearDetailState] = useState<YearsLoadState>("loading");
  const [yearError, setYearError] = useState<string | null>(null);
  const [yearDetailError, setYearDetailError] = useState<string | null>(null);
  const [yearReloadToken, setYearReloadToken] = useState(0);
  const [yearQueueBusy, setYearQueueBusy] = useState(false);
  const [yearQueueMessage, setYearQueueMessage] = useState<string | null>(null);
  const [selectedYearAlbum, setSelectedYearAlbum] = useState<YearAlbum | null>(null);
  const [yearAlbumTracks, setYearAlbumTracks] = useState<Track[]>([]);
  const [yearAlbumBusy, setYearAlbumBusy] = useState(false);
  const [chartSelection, setChartSelection] = useState<ChartSelectionContext | null>(null);
  const [chartPlaybackBusy, setChartPlaybackBusy] = useState(false);
  const [chartReloadToken, setChartReloadToken] = useState(0);
  const [ratingsOverview, setRatingsOverview] = useState<RatingsOverview | null>(null);
  const [ratingsPage, setRatingsPage] = useState<RatingAlbumPage | null>(null);
  const [ratingsLoadState, setRatingsLoadState] = useState<RatingsLoadState>("loading");
  const [ratingsPageState, setRatingsPageState] = useState<RatingsLoadState>("loading");
  const [ratingsError, setRatingsError] = useState<string | null>(null);
  const [ratingsPageError, setRatingsPageError] = useState<string | null>(null);
  const [ratingsReloadToken, setRatingsReloadToken] = useState(0);
  const [ratingsCompletion, setRatingsCompletion] = useState<CompletionKind>("almostComplete");
  const [selectedRatingAlbum, setSelectedRatingAlbum] = useState<RatingAlbum | null>(null);
  const [ratingAlbumTracks, setRatingAlbumTracks] = useState<Track[]>([]);
  const [ratingsQueueBusy, setRatingsQueueBusy] = useState(false);
  const [ratingsQueueMessage, setRatingsQueueMessage] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsInitialTab, setSettingsInitialTab] = useState<SettingsTab>("audio");
  const [shortcutStatus, setShortcutStatus] = useState<GlobalShortcutStatus | null>(null);
  const [shortcutSaving, setShortcutSaving] = useState(false);
  const [shortcutError, setShortcutError] = useState<string | null>(null);
  const [audioStatus, setAudioStatus] = useState<AudioSettingsStatus | null>(null);
  const [audioSaving, setAudioSaving] = useState(false);
  const [audioError, setAudioError] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const exploreRequestRef = useRef(0);
  const albumRequestRef = useRef(0);
  const artistRequestRef = useRef(0);
  const reviewRequestRef = useRef(0);
  const historyRequestRef = useRef(0);
  const genreIndexRequestRef = useRef(0);
  const genreDetailRequestRef = useRef(0);
  const genreQueueRequestRef = useRef(0);
  const publisherOverviewRequestRef = useRef(0);
  const publisherDetailRequestRef = useRef(0);
  const publisherAlbumRequestRef = useRef(0);
  const publisherLoadedSearchRef = useRef<string | null>(null);
  const publisherDetailRef = useRef<PublisherDetail | null>(publisherDetail);
  const selectedPublisherAlbumRef = useRef<PublisherAlbum | null>(selectedPublisherAlbum);
  const yearOverviewRequestRef = useRef(0);
  const yearDetailRequestRef = useRef(0);
  const yearAlbumRequestRef = useRef(0);
  const yearLoadedTokenRef = useRef(-1);
  const yearDetailRef = useRef<YearDetail | null>(yearDetail);
  const selectedYearAlbumRef = useRef<YearAlbum | null>(selectedYearAlbum);
  const ratingsRequestRef = useRef(0);
  const ratingsPageRequestRef = useRef(0);
  const ratingsAlbumRequestRef = useRef(0);
  const ratingsLoadedTokenRef = useRef(-1);
  const ratingsPreserveInspectorTokenRef = useRef<number | null>(null);
  const selectedRatingAlbumRef = useRef<RatingAlbum | null>(selectedRatingAlbum);
  const genreRefillRunningRef = useRef(false);
  const reconciliationRunningRef = useRef(false);
  const catalogSyncNoticeRef = useRef<CatalogSync | null>(null);
  const latestTagProjectionTokenRef = useRef(0);
  const latestTrackProjectionTokensRef = useRef<ReadonlyMap<string, number>>(new Map());
  const latestCatalogSyncTokenRef = useRef(0);
  const appFocusedRef = useRef(typeof document === "undefined" ? true : document.hasFocus());
  const catalogRevisionRef = useRef<string | null>(null);
  const catalogRefreshPromiseRef = useRef<Promise<boolean> | null>(null);
  const catalogRefreshRequestedRef = useRef(false);
  const appMountedRef = useRef(true);
  const selectedTrackRef = useRef<Track | null>(selectedTrack);
  const explorerRestorationPendingRef = useRef(true);
  const inspectorViewRef = useRef(inspectorView);
  const inspectorArtistNameRef = useRef(inspectorArtistName);
  const openArtistInspectorRef = useRef<(artistName: string) => void>(() => undefined);
  const inlineSaveRef = useRef<Set<string>>(new Set());
  const shortcutResultHandlerRef = useRef<(result: GlobalShortcutResult) => void>(() => undefined);
  const updater = useAuroraUpdater();
  const playback = usePlayback();
  const appendPlayback = playback.append;
  const rebindPlaybackCatalog = playback.rebindCatalog;
  const selectedGenreRef = useRef(selectedGenre);
  selectedGenreRef.current = selectedGenre;
  selectedTrackRef.current = selectedTrack;
  inspectorViewRef.current = inspectorView;
  inspectorArtistNameRef.current = inspectorArtistName;
  publisherDetailRef.current = publisherDetail;
  selectedPublisherAlbumRef.current = selectedPublisherAlbum;
  yearDetailRef.current = yearDetail;
  selectedYearAlbumRef.current = selectedYearAlbum;
  selectedRatingAlbumRef.current = selectedRatingAlbum;

  useEffect(() => {
    appMountedRef.current = true;
    return () => {
      appMountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    saveLayoutPreferences(layoutPreferences);
  }, [layoutPreferences]);

  useEffect(() => {
    saveDisplayPreferences(displayPreferences);
  }, [displayPreferences]);

  useEffect(() => {
    saveViewPreferences({
      activeNav,
      explorerView,
      explorerFilters,
      inspectorView,
      tagSelectionKind,
      selectedAlbumId,
    });
  }, [activeNav, explorerFilters, explorerView, inspectorView, selectedAlbumId, tagSelectionKind]);

  useEffect(() => {
    let cancelled = false;
    const refresh = () => {
      void loadLaptopModeStatus()
        .then((status) => {
          if (cancelled) return;
          setLaptopModeStatus(status);
          setLaptopModeError(null);
        })
        .catch((error: unknown) => {
          if (!cancelled) setLaptopModeError(error instanceof Error ? error.message : String(error));
        });
    };
    refresh();
    const interval = window.setInterval(refresh, 5_000);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: () => void = () => undefined;
    void listenForGlobalShortcutResults((result) => shortcutResultHandlerRef.current(result))
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch((error: unknown) => console.warn("Aurora could not listen for shortcut results", error));
    return () => {
      cancelled = true;
      unlisten();
    };
  }, []);

  useEffect(() => {
    if (!settingsOpen) return;
    let cancelled = false;
    void Promise.allSettled([loadGlobalShortcutSettings(), loadAudioSettings()]).then(([shortcuts, audio]) => {
      if (cancelled) return;
      if (shortcuts.status === "fulfilled") setShortcutStatus(shortcuts.value);
      else {
        const message = shortcuts.reason instanceof Error ? shortcuts.reason.message : String(shortcuts.reason);
        setShortcutStatus({
          enabled: true,
          registered: false,
          platformAvailable: true,
          error: message,
          warning: null,
          bindings: defaultShortcutBindings,
        });
      }
      if (audio.status === "fulfilled") setAudioStatus(audio.value);
      else {
        const message = audio.reason instanceof Error ? audio.reason.message : String(audio.reason);
        setAudioStatus({
          settings: { outputDeviceId: "system-default", replayGainMode: "off" },
          devices: [],
          activeDeviceId: null,
          activeDeviceLabel: null,
          usingFallback: false,
          message: null,
          error: message,
        });
      }
    });
    return () => { cancelled = true; };
  }, [settingsOpen]);

  useEffect(() => {
    let cancelled = false;
    void loadLibrarySnapshot()
      .then((nextSnapshot) => {
        if (cancelled) return;
        if (
          catalogRevisionRef.current !== null
          && nextSnapshot.catalogRevision !== catalogRevisionRef.current
        ) return;
        catalogRevisionRef.current ??= nextSnapshot.catalogRevision;
        setSnapshot(nextSnapshot);
        setSelectedTrack((current) => current ?? nextSnapshot.tracks[0] ?? null);
      })
      .catch((error: unknown) => {
        if (!cancelled) setLoadError(error instanceof Error ? error.message : String(error));
      });
    return () => { cancelled = true; };
  }, [reloadToken]);

  useEffect(() => {
    function focusSearch(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLocaleLowerCase() === "k") {
        event.preventDefault();
        searchRef.current?.focus();
      }
    }
    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  }, []);

  useEffect(() => {
    const trackKey = selectedTrack?.trackKey;
    if (!trackKey) return;
    let cancelled = false;
    const refresh = () => {
      void loadTrackHistoryInsight(trackKey)
        .then((value) => {
          if (!cancelled) setTrackHistory({ trackKey, value });
        })
        .catch(() => undefined);
    };
    refresh();
    const interval = window.setInterval(refresh, 15_000);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [selectedTrack?.trackKey]);

  const libraryReady = snapshot !== null;

  const refreshCatalogIfChanged = useCallback((
    isCurrent: () => boolean = () => true,
  ): Promise<boolean> => {
    const shouldContinue = () => appMountedRef.current && isCurrent();
    if (!shouldContinue()) return Promise.resolve(false);
    const runningRefresh = catalogRefreshPromiseRef.current;
    if (runningRefresh) {
      catalogRefreshRequestedRef.current = true;
      return runningRefresh;
    }

    const refreshOnce = async (): Promise<boolean> => {
      for (let attempt = 0; attempt < 2; attempt += 1) {
        const projectionTokenAtStart = latestTagProjectionTokenRef.current;
        const revision = await loadCatalogRevision();
        if (!shouldContinue()) return false;
        const previousRevision = catalogRevisionRef.current;
        if (previousRevision === null) {
          catalogRevisionRef.current = revision;
          return false;
        }
        if (revision === previousRevision) return false;

        const reboundResult = await rebindPlaybackCatalog();
        if (!shouldContinue() || reboundResult === null) return false;

        const nextSnapshot = await loadLibrarySnapshot();
        if (!shouldContinue()) return false;
        if (!catalogRefreshIsConsistent(
          revision,
          reboundResult.catalogRevision,
          nextSnapshot.catalogRevision,
        )) {
          if (attempt === 0) continue;
          throw new Error("The Music Library catalog changed again during Aurora's refresh.");
        }
        if (latestTagProjectionTokenRef.current !== projectionTokenAtStart) {
          if (attempt === 0) continue;
          catalogRefreshRequestedRef.current = true;
          return false;
        }

        const rebound = reboundResult.playback;
        setLoadError(null);
        setSnapshot(nextSnapshot);
        const selectedKey = selectedTrackRef.current?.trackKey;
        if (selectedKey) {
          const reboundSelection = rebound.queue.find((track) => track.trackKey === selectedKey)
            ?? nextSnapshot.tracks.find((track) => track.trackKey === selectedKey);
          if (reboundSelection) setSelectedTrack(reboundSelection);
        }
        catalogRevisionRef.current = revision;

        setExplorerReloadToken((value) => value + 1);
        setReviewReloadToken((value) => value + 1);
        setHistoryReloadToken((value) => value + 1);
        setGenreIndexReloadToken((value) => value + 1);
        setGenreDetailReloadToken((value) => value + 1);
        setPublisherReloadToken((value) => value + 1);
        setYearReloadToken((value) => value + 1);
        setChartReloadToken((value) => value + 1);
        setSyncMessage("Music Library import detected · refreshed Aurora");
        const artistName = inspectorArtistNameRef.current;
        if (inspectorViewRef.current === "artist" && artistName) {
          openArtistInspectorRef.current(artistName);
        }
        return true;
      }
      return false;
    };

    const runRefresh = async (): Promise<boolean> => {
      let refreshed = false;
      do {
        catalogRefreshRequestedRef.current = false;
        refreshed = await refreshOnce() || refreshed;
      } while (catalogRefreshRequestedRef.current && shouldContinue());
      return refreshed;
    };

    const refreshTask = runRefresh().finally(() => {
      if (catalogRefreshPromiseRef.current === refreshTask) {
        catalogRefreshPromiseRef.current = null;
      }
    });
    catalogRefreshPromiseRef.current = refreshTask;
    return refreshTask;
  }, [rebindPlaybackCatalog]);

  const handleCatalogSync = useCallback(async (
    sync: CatalogSync | null | undefined,
    announceSuccess = false,
  ): Promise<void> => {
    if (!sync) return;
    const syncDecision = advanceCatalogProjectionToken(
      latestCatalogSyncTokenRef.current,
      sync.projectionToken,
    );
    if (!syncDecision.accepted) return;
    latestCatalogSyncTokenRef.current = syncDecision.latestToken;
    const previous = catalogSyncNoticeRef.current;
    const wasPending = catalogSyncNeedsRetry(previous);
    const needsRetry = catalogSyncNeedsRetry(sync);
    if (needsRetry || announceSuccess || wasPending) {
      catalogSyncNoticeRef.current = sync;
      setCatalogSyncNotice(sync);
    } else if (!previous) {
      catalogSyncNoticeRef.current = sync;
    }
    try {
      await refreshCatalogIfChanged();
    } catch (error) {
      console.warn("Aurora could not check Music Library for partial sync updates yet", error);
    }
  }, [refreshCatalogIfChanged]);

  const acceptTrackProjectionKeys = useCallback((
    trackKeys: readonly string[],
    projectionToken: number | null | undefined,
  ) => {
    const decision = advanceCatalogTrackProjectionTokens(
      latestTagProjectionTokenRef.current,
      latestTrackProjectionTokensRef.current,
      projectionToken,
      trackKeys,
    );
    latestTagProjectionTokenRef.current = decision.latestToken;
    latestTrackProjectionTokensRef.current = decision.latestTrackTokens;
    return decision;
  }, []);

  useEffect(() => {
    if (!libraryReady) return;
    let cancelled = false;
    const isCurrent = () => !cancelled;
    const refreshQuietly = () => {
      void refreshCatalogIfChanged(isCurrent).catch((error: unknown) => {
        console.warn("Aurora could not check the Music Library catalog revision", error);
      });
    };
    const initialRefresh = window.setTimeout(refreshQuietly, 0);
    const interval = window.setInterval(refreshQuietly, 5_000);
    const refreshOnFocus = refreshQuietly;
    window.addEventListener("focus", refreshOnFocus);
    return () => {
      cancelled = true;
      window.clearTimeout(initialRefresh);
      window.clearInterval(interval);
      window.removeEventListener("focus", refreshOnFocus);
    };
  }, [libraryReady, refreshCatalogIfChanged]);

  useEffect(() => {
    const candidates = [
      ...(snapshot?.tracks ?? []),
      ...explorerTracks,
      ...albumTracks,
      ...yearAlbumTracks,
      ...ratingAlbumTracks,
      ...publisherAlbumTracks,
      ...(genreDetail?.highlights ?? []),
    ];
    if (candidates.length === 0) return;
    const timer = window.setTimeout(() => {
      setSelectedTrack((current) => {
        if (!current) return current;
        return candidates.find((track) => track.trackKey === current.trackKey) ?? current;
      });
    }, 0);
    return () => window.clearTimeout(timer);
  }, [
    snapshot,
    explorerTracks,
    albumTracks,
    yearAlbumTracks,
    ratingAlbumTracks,
    publisherAlbumTracks,
    genreDetail,
  ]);

  useEffect(() => {
    if (
      !libraryReady
      || activeNav === "Observatory"
      || activeNav === "Charts"
      || activeNav === "History"
      || activeNav === "Genres"
      || activeNav === "Publishers"
      || activeNav === "Years"
      || activeNav === "Ratings"
    ) return;
    const restoringStoredView = explorerRestorationPendingRef.current;
    const restoredAlbumId = restoringStoredView && explorerView === "albums"
      ? initialViewPreferences.selectedAlbumId
      : null;
    const requestId = ++exploreRequestRef.current;
    let cancelled = false;
    albumRequestRef.current += 1;
    const clearDetailTimer = window.setTimeout(() => {
      if (cancelled) return;
      setIsLoadingMore(false);
      if (!restoredAlbumId) setSelectedAlbumId(null);
      setAlbumTracks([]);
      setAlbumTracksTruncated(false);
    }, 0);
    const timer = window.setTimeout(() => {
      setExplorerLoadState("loading");
      setExplorerError(null);
      setExplorerCursor(null);
      void loadExplorerPage(explorerView, explorerFilters)
        .then((page) => {
          if (cancelled || requestId !== exploreRequestRef.current) return;
          setExplorerTracks(page.tracks);
          setExplorerAlbums(page.albums);
          setExplorerArtists(page.artists);
          setExplorerCursor(page.nextCursor);
          setExplorerCount({ key: explorerCountKey(explorerView, explorerFilters), total: page.totalCount });
          setExplorerLoadState("ready");
          if (restoredAlbumId && page.albums.some((album) => album.id === restoredAlbumId)) {
            const albumDetailRequestId = ++albumRequestRef.current;
            setSelectedAlbumId(restoredAlbumId);
            setAlbumDetailState("loading");
            void loadAlbumDetail(restoredAlbumId)
              .then((detail) => {
                if (albumDetailRequestId !== albumRequestRef.current) return;
                setExplorerAlbums((current) => current.map((album) => album.id === detail.album.id ? detail.album : album));
                setAlbumTracks(detail.tracks);
                setAlbumTracksTruncated(detail.tracksTruncated);
                setSelectedTrack(detail.tracks[0] ?? null);
                setAlbumDetailState("ready");
              })
              .catch((error: unknown) => {
                if (albumDetailRequestId !== albumRequestRef.current) return;
                console.warn("Aurora could not restore album details", error);
                setAlbumDetailState("error");
              });
          } else if (restoringStoredView) {
            setSelectedAlbumId(null);
          }
          explorerRestorationPendingRef.current = false;
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== exploreRequestRef.current) return;
          explorerRestorationPendingRef.current = false;
          setExplorerError(error instanceof Error ? error.message : String(error));
          setExplorerLoadState("error");
        });
    }, explorerFilters.query.trim() ? 160 : 0);
    return () => {
      cancelled = true;
      window.clearTimeout(clearDetailTimer);
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, explorerView, explorerFilters, explorerReloadToken, initialViewPreferences.selectedAlbumId]);

  useEffect(() => {
    if (
      libraryReady
      && ["Observatory", "Charts", "History", "Genres", "Publishers", "Years", "Ratings"].includes(activeNav)
    ) explorerRestorationPendingRef.current = false;
  }, [activeNav, libraryReady]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Genres") return;
    const requestId = ++genreIndexRequestRef.current;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setGenreIndexState("loading");
      setGenreIndexError(null);
      void loadGenreIndex()
        .then((items) => {
          if (cancelled || requestId !== genreIndexRequestRef.current) return;
          setGenreAtlasGenres(items);
          setSelectedGenre((current) => current && items.some((item) => item.name === current)
            ? current
            : items[0]?.name ?? null);
          setGenreIndexState("ready");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== genreIndexRequestRef.current) return;
          setGenreIndexError(error instanceof Error ? error.message : String(error));
          setGenreIndexState("error");
        });
    }, 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, genreIndexReloadToken]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Genres" || !selectedGenre) return;
    const requestId = ++genreDetailRequestRef.current;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setGenreDetailState("loading");
      setGenreDetailError(null);
      void loadGenreDetail(selectedGenre)
        .then((detail) => {
          if (cancelled || requestId !== genreDetailRequestRef.current) return;
          setGenreDetail(detail);
          setGenreDetailState("ready");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== genreDetailRequestRef.current) return;
          setGenreDetailError(error instanceof Error ? error.message : String(error));
          setGenreDetailState("error");
        });
    }, 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, selectedGenre, genreDetailReloadToken]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Publishers") return;
    const requestId = ++publisherOverviewRequestRef.current;
    publisherDetailRequestRef.current += 1;
    publisherAlbumRequestRef.current += 1;
    const preserveSelection = publisherLoadedSearchRef.current === publisherSearch;
    const previousPublisher = preserveSelection
      ? publisherDetailRef.current?.publisher.name ?? null
      : null;
    const previousAlbumId = preserveSelection
      ? selectedPublisherAlbumRef.current?.id ?? null
      : null;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setPublisherLoadState("loading");
      setPublisherDetailState("loading");
      setPublisherError(null);
      setPublisherDetailError(null);
      setPublisherQueueMessage(null);
      const detailRequest = previousPublisher
        ? loadPublisherDetail(previousPublisher).catch(() => null)
        : Promise.resolve(null);
      void Promise.all([loadPublisherOverview(publisherSearch), detailRequest])
        .then(([overview, refreshedDetail]) => {
          if (cancelled || requestId !== publisherOverviewRequestRef.current) return;
          const detail = refreshedDetail ?? overview.initialDetail;
          publisherLoadedSearchRef.current = publisherSearch;
          setPublisherOverview(overview);
          setPublisherDetail(detail);
          setPublisherLoadState("ready");
          setPublisherDetailState("ready");
          const initialAlbum = detail.albums.find((album) => album.id === previousAlbumId)
            ?? detail.albums[0]
            ?? null;
          setSelectedPublisherAlbum(initialAlbum);
          setPublisherAlbumTracks([]);
          if (!initialAlbum) return;
          if (!preserveSelection) setInspectorView("album");
          const albumRequestId = ++publisherAlbumRequestRef.current;
          void loadPublisherAlbumTracks(initialAlbum)
            .then((tracks) => {
              if (!cancelled && albumRequestId === publisherAlbumRequestRef.current) {
                setPublisherAlbumTracks(tracks);
              }
            })
            .catch(() => undefined);
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== publisherOverviewRequestRef.current) return;
          setPublisherError(error instanceof Error ? error.message : String(error));
          setPublisherLoadState("error");
          setPublisherDetailState("error");
        });
    }, publisherSearch.trim() ? 160 : 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, publisherReloadToken, publisherSearch]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Years") return;
    if (yearOverview && yearLoadedTokenRef.current === yearReloadToken) {
      setYearLoadState("ready");
      return;
    }
    const requestId = ++yearOverviewRequestRef.current;
    yearDetailRequestRef.current += 1;
    const preserveSelection = yearLoadedTokenRef.current >= 0;
    const previousSelection = preserveSelection ? yearDetailRef.current?.selection ?? null : null;
    const previousAlbumId = preserveSelection ? selectedYearAlbumRef.current?.id ?? null : null;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setYearLoadState("loading");
      setYearDetailState("loading");
      setYearError(null);
      setYearDetailError(null);
      setYearQueueMessage(null);
      const detailRequest = previousSelection
        ? loadYearDetail(previousSelection).catch(() => null)
        : Promise.resolve(null);
      void Promise.all([loadYearOverview(), detailRequest])
        .then(([overview, refreshedDetail]) => {
          if (cancelled || requestId !== yearOverviewRequestRef.current) return;
          const detail = refreshedDetail ?? overview.initialDetail;
          setYearOverview(overview);
          yearLoadedTokenRef.current = yearReloadToken;
          setYearDetail(detail);
          setYearLoadState("ready");
          setYearDetailState("ready");
          const initialAlbum = detail.albums.find((album) => album.id === previousAlbumId)
            ?? detail.albums[0]
            ?? null;
          setSelectedYearAlbum(initialAlbum);
          setYearAlbumTracks([]);
          if (!initialAlbum) return;
          if (!preserveSelection) setInspectorView("album");
          const albumRequestId = ++yearAlbumRequestRef.current;
          void loadYearAlbumTracks(initialAlbum)
            .then((tracks) => {
              if (albumRequestId === yearAlbumRequestRef.current) setYearAlbumTracks(tracks);
            })
            .catch(() => undefined);
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== yearOverviewRequestRef.current) return;
          setYearError(error instanceof Error ? error.message : String(error));
          setYearLoadState("error");
          setYearDetailState("error");
        });
    }, 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, yearOverview, yearReloadToken]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Ratings") return;
    const requestId = ++ratingsRequestRef.current;
    ratingsPageRequestRef.current += 1;
    ratingsAlbumRequestRef.current += 1;
    const preserveSelection = ratingsLoadedTokenRef.current >= 0;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (!preserveSelection) {
        setRatingsLoadState("loading");
        setRatingsPageState("loading");
        setRatingsPage(null);
      }
      setRatingsError(null);
      setRatingsPageError(null);
      setRatingsQueueMessage(null);
      void loadRatingsOverview()
        .then((overview) => {
          if (cancelled || requestId !== ratingsRequestRef.current) return;
          setRatingsOverview(overview);
          ratingsPreserveInspectorTokenRef.current = preserveSelection
            ? ratingsReloadToken
            : null;
          ratingsLoadedTokenRef.current = ratingsReloadToken;
          setRatingsLoadState("ready");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== ratingsRequestRef.current) return;
          setRatingsError(error instanceof Error ? error.message : String(error));
          setRatingsLoadState("error");
          setRatingsPageState("error");
        });
    }, 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, ratingsReloadToken]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Ratings" || !ratingsOverview) return;
    if (ratingsLoadedTokenRef.current !== ratingsReloadToken) return;
    const pageRequestId = ++ratingsPageRequestRef.current;
    ratingsAlbumRequestRef.current += 1;
    const preserveSelection = ratingsPreserveInspectorTokenRef.current === ratingsReloadToken;
    const previousAlbumId = preserveSelection ? selectedRatingAlbumRef.current?.id ?? null : null;
    let cancelled = false;
    if (!preserveSelection) setRatingsPageState("loading");
    setRatingsPageError(null);
    const request = ratingsCompletion === "almostComplete"
      ? Promise.resolve(ratingsOverview.initialPage)
      : loadRatingAlbumPage(ratingsCompletion);
    void request
      .then((page) => {
          if (cancelled || pageRequestId !== ratingsPageRequestRef.current) return;
          setRatingsPage(page);
          setRatingsPageState("ready");
          const initialAlbum = page.albums.find((album) => album.id === previousAlbumId)
            ?? page.albums[0]
            ?? null;
          setSelectedRatingAlbum(initialAlbum);
          setRatingAlbumTracks([]);
          ratingsPreserveInspectorTokenRef.current = null;
          if (!initialAlbum) return;
          if (!preserveSelection) setInspectorView("album");
        const albumRequestId = ++ratingsAlbumRequestRef.current;
        void loadRatingAlbumTracks(initialAlbum)
          .then((tracks) => {
            if (!cancelled && albumRequestId === ratingsAlbumRequestRef.current) {
              setRatingAlbumTracks(tracks);
            }
          })
          .catch(() => undefined);
      })
      .catch((error: unknown) => {
        if (cancelled || pageRequestId !== ratingsPageRequestRef.current) return;
        setRatingsPageError(error instanceof Error ? error.message : String(error));
        setRatingsPageState("error");
      });
    return () => {
      cancelled = true;
    };
  }, [activeNav, libraryReady, ratingsCompletion, ratingsOverview, ratingsReloadToken]);

  useEffect(() => {
    if (!genreRadioSession) return;
    if (playback.state.queue.length === 0 || playback.state.currentIndex === null) return;
    const remaining = playback.state.queue.length - playback.state.currentIndex - 1;
    if (remaining >= 20 || genreRefillRunningRef.current) return;
    const requestId = ++genreQueueRequestRef.current;
    genreRefillRunningRef.current = true;
    const excluded = playback.state.queue.map((track) => track.trackKey);
    void loadGenreQueue({
      genre: genreRadioSession.genre,
      mode: genreRadioSession.mode,
      limit: 100,
      excludeTrackKeys: excluded,
    })
      .then(async (tracks) => {
        if (requestId !== genreQueueRequestRef.current) return;
        if (tracks.length === 0) {
          setGenreQueueMessage(`Aurora reached the end of this ${genreRadioSession.genre} expedition.`);
          return;
        }
        const next = await appendPlayback(tracks);
        if (requestId === genreQueueRequestRef.current && next) {
          setGenreQueueMessage(`Added ${formatCount(tracks.length)} more ${genreRadioSession.genre} tracks.`);
        }
      })
      .catch((error: unknown) => {
        if (requestId === genreQueueRequestRef.current) {
          setGenreQueueMessage(`Could not refill Genre Radio: ${error instanceof Error ? error.message : String(error)}`);
        }
      })
      .finally(() => {
        if (requestId === genreQueueRequestRef.current) genreRefillRunningRef.current = false;
      });
  }, [appendPlayback, genreRadioSession, playback.state.currentIndex, playback.state.queue]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Observatory") return;
    const requestId = ++reviewRequestRef.current;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setReviewLoadState("loading");
      setReviewError(null);
      setReviewCursor(null);
      void loadArtistReviewPage({ pageSize: 50, filter: reviewFilter, search: reviewSearch.trim() || undefined })
        .then((page) => {
          if (cancelled || requestId !== reviewRequestRef.current) return;
          setReviewItems(page.items);
          setReviewCursor(page.nextCursor);
          setReviewLoadState("ready");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== reviewRequestRef.current) return;
          setReviewError(error instanceof Error ? error.message : String(error));
          setReviewLoadState("error");
        });
    }, reviewSearch.trim() ? 160 : 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, reviewFilter, reviewSearch, reviewReloadToken]);

  useEffect(() => {
    if (!libraryReady || (activeNav !== "History" && activeNav !== "Universe")) return;
    const requestId = ++historyRequestRef.current;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (activeNav === "History") setHistoryLoadState("loading");
      setHistoryError(null);
      const startedAfterMs = activeNav === "History" && historyDateRange !== "all"
        ? Date.now() - Number(historyDateRange) * 86_400_000
        : undefined;
      void loadHistoryPage({
        pageSize: activeNav === "History" ? 50 : 5,
        search: activeNav === "History" ? historySearch.trim() || undefined : undefined,
        outcome: activeNav === "History" ? historyOutcome : "all",
        deviceId: activeNav === "History" ? historyDeviceId ?? undefined : undefined,
        startedAfterMs,
      })
        .then((page) => {
          if (cancelled || requestId !== historyRequestRef.current) return;
          setHistoryPage(page);
          setHistoryLoadState("ready");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== historyRequestRef.current) return;
          setHistoryError(error instanceof Error ? error.message : String(error));
          setHistoryLoadState("error");
        });
    }, activeNav === "History" && historySearch.trim() ? 160 : 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [
    activeNav,
    libraryReady,
    historySearch,
    historyOutcome,
    historyDeviceId,
    historyDateRange,
    historyReloadToken,
  ]);

  useEffect(() => {
    if (activeNav !== "History") return;
    const interval = window.setInterval(() => {
      setHistoryReloadToken((value) => value + 1);
    }, 15_000);
    return () => window.clearInterval(interval);
  }, [activeNav]);

  const applyReconciliationChanges = useCallback((changes: TagReconciliationChange[]) => {
    if (changes.length === 0) return;
    const byTrackKey = new Map(changes.map((change) => [change.trackKey, change]));
    const reconcile = (track: Track) => {
      const change = byTrackKey.get(track.trackKey);
      return change ? trackWithReconciledTags(track, change) : track;
    };
    setExplorerTracks((current) => current.map(reconcile));
    setAlbumTracks((current) => current.map(reconcile));
    setYearAlbumTracks((current) => current.map(reconcile));
    setRatingAlbumTracks((current) => current.map(reconcile));
    setPublisherAlbumTracks((current) => current.map(reconcile));
    setGenreDetail((current) => current ? {
      ...current,
      highlights: current.highlights.map(reconcile),
    } : current);
    setSelectedTrack((current) => current ? reconcile(current) : current);
    setSnapshot((current) => current ? { ...current, tracks: current.tracks.map(reconcile) } : current);
  }, []);

  const refreshExternalTagChanges = useCallback(async () => {
    if (reconciliationRunningRef.current) return;
    reconciliationRunningRef.current = true;
    try {
      const report = await reconcilePendingTags();
      setReconciliationHasMore(report.hasMore);
      const projection = acceptTrackProjectionKeys(
        report.changes.map((change) => change.trackKey),
        report.projectionToken,
      );
      applyReconciliationChanges(report.changes.filter((change) => (
        projection.acceptedTrackKeys.has(change.trackKey)
      )));
      await handleCatalogSync(report.catalogSync ? {
        ...report.catalogSync,
        projectionToken: report.projectionToken,
      } : undefined);
      if (report.externalChanges > 0) {
        setSyncMessage(`Refreshed ${formatCount(report.externalChanges)} external tag ${report.externalChanges === 1 ? "change" : "changes"}`);
      } else if (report.issues.length > 0 || report.hasMore) {
        setSyncMessage(report.issues.length === 1 && report.issues[0]?.message
          ? report.issues[0].message
          : `${formatCount(report.issues.length)} tag ${report.issues.length === 1 ? "item needs" : "items need"} attention`);
      } else if (!catalogSyncNeedsRetry(report.catalogSync ?? catalogSyncNoticeRef.current)) {
        setSyncMessage(null);
      }
    } catch (error) {
      console.warn("Aurora could not reconcile pending tags", error);
      setReconciliationHasMore(true);
      setSyncMessage("Tag and Music Library refresh will retry automatically");
    } finally {
      reconciliationRunningRef.current = false;
    }
  }, [acceptTrackProjectionKeys, applyReconciliationChanges, handleCatalogSync]);

  useEffect(() => {
    if (!libraryReady) return;
    const initialRefresh = window.setTimeout(() => void refreshExternalTagChanges(), 0);
    const refreshOnFocus = () => {
      appFocusedRef.current = true;
      void refreshExternalTagChanges();
    };
    const pauseOnBlur = () => {
      appFocusedRef.current = false;
    };
    appFocusedRef.current = document.hasFocus();
    window.addEventListener("focus", refreshOnFocus);
    window.addEventListener("blur", pauseOnBlur);
    return () => {
      window.clearTimeout(initialRefresh);
      window.removeEventListener("focus", refreshOnFocus);
      window.removeEventListener("blur", pauseOnBlur);
    };
  }, [libraryReady, reloadToken, refreshExternalTagChanges]);

  const catalogSyncRetryPending = catalogSyncNeedsRetry(catalogSyncNotice) || reconciliationHasMore;
  useEffect(() => {
    if (!libraryReady || !catalogSyncRetryPending) return;
    const interval = window.setInterval(() => {
      if (appFocusedRef.current) void refreshExternalTagChanges();
    }, catalogSyncRetryIntervalMs);
    return () => window.clearInterval(interval);
  }, [catalogSyncRetryPending, libraryReady, refreshExternalTagChanges]);

  useEffect(() => {
    if (!catalogSyncNotice || catalogSyncNeedsRetry(catalogSyncNotice)) return;
    const settledNotice = catalogSyncNotice;
    const timeout = window.setTimeout(() => {
      if (catalogSyncNoticeRef.current !== settledNotice) return;
      catalogSyncNoticeRef.current = null;
      setCatalogSyncNotice(null);
    }, 6_000);
    return () => window.clearTimeout(timeout);
  }, [catalogSyncNotice]);

  function playTrack(
    track: Track,
    queue = albumTracks.some((candidate) => candidate.id === track.id)
      ? albumTracks
      : explorerTracks.length > 0
        ? explorerTracks
        : snapshot?.tracks ?? [],
  ) {
    endGenreQueue();
    selectTrack(track);
    void playback.play(queue, track.id);
  }

  function endGenreQueue() {
    genreQueueRequestRef.current += 1;
    genreRefillRunningRef.current = false;
    setGenreRadioSession(null);
    saveGenreRadioSession(null);
    setGenreQueueMessage(null);
  }

  async function startGenreQueue(mode: GenreQueueMode) {
    if (!selectedGenre || genreQueueBusy) return;
    const requestedGenre = selectedGenre;
    const requestId = ++genreQueueRequestRef.current;
    genreRefillRunningRef.current = false;
    setGenreQueueBusy(mode);
    setGenreQueueMessage(null);
    try {
      const tracks = await loadGenreQueue({
        genre: requestedGenre,
        mode,
        limit: 100,
        excludeTrackKeys: [],
      });
      if (requestId !== genreQueueRequestRef.current || selectedGenreRef.current !== requestedGenre) return;
      if (tracks.length === 0) {
        setGenreQueueMessage(`No ${requestedGenre} tracks match this expedition yet.`);
        return;
      }
      const next = await playback.play(tracks, tracks[0].id);
      if (requestId !== genreQueueRequestRef.current || selectedGenreRef.current !== requestedGenre || !next) return;
      const session: GenreRadioSession = { version: 1, genre: requestedGenre, mode };
      setGenreRadioSession(session);
      saveGenreRadioSession(session);
      setGenreQueueMessage(`Loaded ${formatCount(tracks.length)} tracks. Aurora will refill with bounded batches.`);
    } catch (error) {
      if (requestId === genreQueueRequestRef.current) {
        setGenreQueueMessage(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (requestId === genreQueueRequestRef.current) setGenreQueueBusy(null);
    }
  }

  function selectPublisher(publisher: PublisherSummary) {
    const requestId = ++publisherDetailRequestRef.current;
    publisherAlbumRequestRef.current += 1;
    setPublisherDetailState("loading");
    setPublisherDetailError(null);
    setPublisherQueueMessage(null);
    void loadPublisherDetail(publisher.name)
      .then((detail) => {
        if (requestId !== publisherDetailRequestRef.current) return;
        setPublisherDetail(detail);
        setPublisherDetailState("ready");
        const initialAlbum = detail.albums[0] ?? null;
        setSelectedPublisherAlbum(initialAlbum);
        setPublisherAlbumTracks([]);
        if (initialAlbum) openPublisherAlbum(initialAlbum);
      })
      .catch((error: unknown) => {
        if (requestId !== publisherDetailRequestRef.current) return;
        setPublisherDetailError(error instanceof Error ? error.message : String(error));
        setPublisherDetailState("error");
      });
  }

  function openPublisherAlbum(album: PublisherAlbum) {
    const requestId = ++publisherAlbumRequestRef.current;
    setSelectedPublisherAlbum(album);
    setTagSelectionKind("album");
    if (inspectorViewRef.current !== "tags") setInspectorView("album");
    setPublisherAlbumTracks([]);
    void loadPublisherAlbumTracks(album)
      .then((tracks) => {
        if (requestId === publisherAlbumRequestRef.current) setPublisherAlbumTracks(tracks);
      })
      .catch((error: unknown) => {
        if (requestId === publisherAlbumRequestRef.current) {
          setPublisherDetailError(error instanceof Error ? error.message : String(error));
        }
      });
  }

  function explorePublisher(publisher: string) {
    setActiveNav("Albums");
    expandLibraryNavigation();
    setExplorerView("albums");
    setExplorerFilters({
      ...defaultExplorerFilters,
      query: `publisher:"${publisher.replace(/"/g, '\\"')}"`,
      sort: "releaseYearDesc",
    });
  }

  async function playPublisher(publisher: string) {
    if (publisherQueueBusy) return;
    setPublisherQueueBusy(true);
    setPublisherQueueMessage(null);
    try {
      const tracks = await loadPublisherQueue(publisher, 100);
      if (!tracks.length) {
        setPublisherQueueMessage("No playable tracks were found for this publisher.");
        return;
      }
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) {
        selectTrack(tracks[0]);
        setPublisherQueueMessage(`Loaded ${formatCount(tracks.length)} tracks from ${publisher}.`);
      }
    } catch (error) {
      setPublisherQueueMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setPublisherQueueBusy(false);
    }
  }

  async function playPublisherAlbum(album: PublisherAlbum) {
    if (publisherAlbumBusy) return;
    setPublisherAlbumBusy(true);
    try {
      const tracks = selectedPublisherAlbum?.id === album.id && publisherAlbumTracks.length
        ? publisherAlbumTracks
        : await loadPublisherAlbumTracks(album);
      if (!tracks.length) {
        setPublisherQueueMessage(`${album.title} has no playable tracks.`);
        return;
      }
      setPublisherAlbumTracks(tracks);
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) selectTrack(tracks[0]);
    } catch (error) {
      setPublisherQueueMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setPublisherAlbumBusy(false);
    }
  }

  function selectYear(selection: YearSelection) {
    const requestId = ++yearDetailRequestRef.current;
    setYearDetailState("loading");
    setYearDetailError(null);
    setYearQueueMessage(null);
    void loadYearDetail(selection)
      .then((detail) => {
        if (requestId !== yearDetailRequestRef.current) return;
        setYearDetail(detail);
        setYearDetailState("ready");
        const nextAlbum = detail.albums[0] ?? null;
        setSelectedYearAlbum(nextAlbum);
        setYearAlbumTracks([]);
        if (nextAlbum) openYearAlbum(nextAlbum);
      })
      .catch((error: unknown) => {
        if (requestId !== yearDetailRequestRef.current) return;
        setYearDetailError(error instanceof Error ? error.message : String(error));
        setYearDetailState("error");
      });
  }

  function openYearAlbum(album: YearAlbum) {
    const requestId = ++yearAlbumRequestRef.current;
    setSelectedYearAlbum(album);
    setYearAlbumTracks([]);
    setTagSelectionKind("album");
    if (inspectorViewRef.current !== "tags") setInspectorView("album");
    void loadYearAlbumTracks(album)
      .then((tracks) => {
        if (requestId === yearAlbumRequestRef.current) setYearAlbumTracks(tracks);
      })
      .catch((error: unknown) => {
        if (requestId === yearAlbumRequestRef.current) {
          console.warn("Aurora could not open this year edition", error);
        }
      });
  }

  function exploreYear(selection: YearSelection) {
    setActiveNav("Songs");
    expandLibraryNavigation();
    setExplorerView("tracks");
    setExplorerFilters({
      ...defaultExplorerFilters,
      yearBasis: selection.basis,
      yearFrom: selection.year,
      yearTo: selection.year,
      yearMissing: selection.year === null,
      sort: selection.basis === "release" ? "releaseYearDesc" : "albumAsc",
    });
  }

  async function playYear(selection: YearSelection) {
    if (yearQueueBusy) return;
    setYearQueueBusy(true);
    setYearQueueMessage(null);
    try {
      const tracks = await loadYearQueue(selection, 100);
      if (tracks.length === 0) {
        setYearQueueMessage("No playable tracks were found for this clock selection.");
        return;
      }
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) {
        selectTrack(tracks[0]);
        setYearQueueMessage(`Loaded ${formatCount(tracks.length)} tracks from this ${selection.basis} year.`);
      }
    } catch (error) {
      setYearQueueMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setYearQueueBusy(false);
    }
  }

  async function playYearAlbum(album: YearAlbum) {
    if (yearAlbumBusy) return;
    setYearAlbumBusy(true);
    try {
      const tracks = selectedYearAlbum?.id === album.id && yearAlbumTracks.length
        ? yearAlbumTracks
        : await loadYearAlbumTracks(album);
      if (!tracks.length) {
        setYearQueueMessage(`${album.title} has no playable tracks in the bounded album detail.`);
        return;
      }
      setYearAlbumTracks(tracks);
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) selectTrack(tracks[0]);
    } catch (error) {
      setYearQueueMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setYearAlbumBusy(false);
    }
  }

  async function playChartQueue(tracks: Track[]): Promise<boolean> {
    if (!tracks.length) return false;
    endGenreQueue();
    const next = await playback.play(tracks, tracks[0].id);
    if (next) selectTrack(tracks[0]);
    return Boolean(next);
  }

  async function playChartSelection() {
    if (!chartSelection || chartPlaybackBusy) return;
    setChartPlaybackBusy(true);
    try {
      if (chartSelection.kind === "singles" && chartSelection.entry.matchedTrackId) {
        const track = selectedTrack?.id === chartSelection.entry.matchedTrackId
          ? selectedTrack
          : await loadChartEntryTrack(chartSelection.entry.matchedTrackId);
        await playChartQueue([track]);
        return;
      }
      if (chartSelection.kind === "albums" && chartSelection.entry.matchedAlbumId) {
        const detail = await loadAlbumDetail(chartSelection.entry.matchedAlbumId);
        await playChartQueue(detail.tracks);
      }
    } catch (error) {
      console.warn("Aurora could not play this chart selection", error);
    } finally {
      setChartPlaybackBusy(false);
    }
  }

  function openChartSelectionInLibrary() {
    if (!chartSelection) return;
    expandLibraryNavigation();
    if (chartSelection.kind === "albums") {
      setActiveNav("Albums");
      setExplorerView("albums");
      setExplorerFilters({ ...defaultExplorerFilters, query: chartSelection.entry.title, sort: "yearDesc" });
      return;
    }
    setActiveNav("Songs");
    setExplorerView("tracks");
    setExplorerFilters({ ...defaultExplorerFilters, query: chartSelection.entry.title, sort: "artistAsc" });
  }

  function openRatingAlbum(album: RatingAlbum) {
    const requestId = ++ratingsAlbumRequestRef.current;
    setSelectedRatingAlbum(album);
    setTagSelectionKind("album");
    if (inspectorViewRef.current !== "tags") setInspectorView("album");
    setRatingAlbumTracks([]);
    void loadRatingAlbumTracks(album)
      .then((tracks) => {
        if (requestId === ratingsAlbumRequestRef.current) setRatingAlbumTracks(tracks);
      })
      .catch((error: unknown) => {
        if (requestId === ratingsAlbumRequestRef.current) {
          setRatingsPageError(error instanceof Error ? error.message : String(error));
        }
      });
  }

  async function playRatingCollection(mode: RatingMode, rating: number | null) {
    if (ratingsQueueBusy) return;
    setRatingsQueueBusy(true);
    setRatingsQueueMessage(null);
    try {
      const tracks = await loadRatingCollection(mode, rating, 100);
      if (!tracks.length) {
        setRatingsQueueMessage("No playable tracks matched this rating band.");
        return;
      }
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) {
        selectTrack(tracks[0]);
        setRatingsQueueMessage(`Loaded ${formatCount(tracks.length)} tracks from this ${mode === "tracks" ? "track" : "album"} rating band.`);
      }
    } catch (error) {
      setRatingsQueueMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setRatingsQueueBusy(false);
    }
  }

  async function playRatingAlbumUnrated(album: RatingAlbum) {
    if (ratingsQueueBusy) return;
    setRatingsQueueBusy(true);
    setRatingsQueueMessage(null);
    try {
      const tracks = await loadRatingAlbumQueue(album, true, 100);
      if (!tracks.length) {
        setRatingsQueueMessage(`${album.title} has no unrated tracks left.`);
        return;
      }
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) {
        selectTrack(tracks[0]);
        setRatingsQueueMessage(`Loaded ${formatCount(tracks.length)} unrated tracks from ${album.title}.`);
      }
    } catch (error) {
      setRatingsQueueMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setRatingsQueueBusy(false);
    }
  }

  function exploreRatingCollection(mode: RatingMode, rating: number | null) {
    expandLibraryNavigation();
    if (mode === "tracks") {
      setActiveNav("Songs");
      setExplorerView("tracks");
      setExplorerFilters({
        ...defaultExplorerFilters,
        rating: (rating ?? "unrated") as ExplorerFilters["rating"],
        sort: rating === null ? "newest" : "ratingDesc",
      });
      return;
    }
    setActiveNav("Albums");
    setExplorerView("albums");
    setExplorerFilters({
      ...defaultExplorerFilters,
      rating: (rating ?? "unrated") as ExplorerFilters["rating"],
      sort: rating === null ? "yearDesc" : "ratingDesc",
    });
  }

  async function toggleLaptopMode() {
    if (!laptopModeStatus || laptopModeBusy) return;
    setLaptopModeBusy(true);
    setLaptopModeError(null);
    try {
      setLaptopModeStatus(await updateLaptopMode(!laptopModeStatus.laptopMode));
    } catch (error) {
      setLaptopModeError(error instanceof Error ? error.message : String(error));
    } finally {
      setLaptopModeBusy(false);
    }
  }

  function selectTrack(track: Track) {
    artistRequestRef.current += 1;
    setSelectedTrack(track);
    setTagSelectionKind("track");
    if (inspectorViewRef.current !== "tags") setInspectorView("track");
  }

  function applyTrackChanges(updatedTracks: Track[], sync?: CatalogSync): boolean {
    const projection = acceptTrackProjectionKeys(
      updatedTracks.map((track) => track.trackKey),
      sync?.projectionToken,
    );
    const acceptedTracks = updatedTracks.filter((track) => (
      projection.acceptedTrackKeys.has(track.trackKey)
    ));
    if (acceptedTracks.length === 0) return projection.complete;
    const updatedByKey = new Map(acceptedTracks.map((track) => [track.trackKey, track]));
    const project = (track: Track) => {
      const updated = updatedByKey.get(track.trackKey);
      return updated ? applyEditableTrackTagProjection(track, updated) : track;
    };
    const knownTracks = [
      ...(selectedTrack ? [selectedTrack] : []),
      ...explorerTracks,
      ...albumTracks,
      ...yearAlbumTracks,
      ...ratingAlbumTracks,
      ...publisherAlbumTracks,
      ...(snapshot?.tracks ?? []),
    ];
    const baselines = new Map(knownTracks.map((track) => [track.trackKey, track]));

    setSelectedTrack((current) => current ? project(current) : current);
    setExplorerTracks((current) => current.map(project));
    setAlbumTracks((current) => current.map(project));
    setYearAlbumTracks((current) => current.map(project));
    setRatingAlbumTracks((current) => current.map(project));
    setPublisherAlbumTracks((current) => current.map(project));
    setGenreDetail((current) => {
      if (!current) return current;
      const summary = acceptedTracks.reduce((next, updated) => {
        const baseline = baselines.get(updated.trackKey);
        return baseline ? genreSummaryWithTrackChange(next, baseline, updated) : next;
      }, current.summary);
      return { ...current, summary, highlights: current.highlights.map(project) };
    });
    setGenreAtlasGenres((current) => current.map((summary) => acceptedTracks.reduce((next, updated) => {
      const baseline = baselines.get(updated.trackKey);
      return baseline ? genreSummaryWithTrackChange(next, baseline, updated) : next;
    }, summary)));
    acceptedTracks.forEach((track) => playback.refreshTrack(track, true));
    setSnapshot((current) => {
      if (!current) return current;
      const deltas = acceptedTracks.reduce((totals, updated) => {
        const baseline = baselines.get(updated.trackKey);
        if (!baseline) return totals;
        totals.loved += Number(updated.loved) - Number(baseline.loved);
        totals.rated += Number(updated.rating !== null) - Number(baseline.rating !== null);
        return totals;
      }, { loved: 0, rated: 0 });
      return {
        ...current,
        summary: {
          ...current.summary,
          loved: Math.max(0, current.summary.loved + deltas.loved),
          rated: Math.max(0, current.summary.rated + deltas.rated),
        },
        tracks: current.tracks.map(project),
      };
    });
    return projection.complete;
  }

  function applyTrackChange(updated: Track, previous?: Track, updateSelected = true) {
    const baseline = previous ?? (selectedTrack?.trackKey === updated.trackKey ? selectedTrack : undefined);
    const refreshMatchingTrack = (track: Track) => track.trackKey === updated.trackKey
      ? applyTrackTagProjection(track, updated)
      : track;
    if (updateSelected) {
      setSelectedTrack((current) => current ? refreshMatchingTrack(current) : current);
    }
    setExplorerTracks((current) => current.map(refreshMatchingTrack));
    setAlbumTracks((current) => current.map(refreshMatchingTrack));
    setYearAlbumTracks((current) => current.map(refreshMatchingTrack));
    setRatingAlbumTracks((current) => current.map(refreshMatchingTrack));
    setPublisherAlbumTracks((current) => current.map(refreshMatchingTrack));
    setGenreDetail((current) => {
      if (!current) return current;
      return {
        ...current,
        summary: baseline ? genreSummaryWithTrackChange(current.summary, baseline, updated) : current.summary,
        highlights: current.highlights.map(refreshMatchingTrack),
      };
    });
    if (baseline) {
      setGenreAtlasGenres((current) => current.map((summary) => genreSummaryWithTrackChange(summary, baseline, updated)));
    }
    playback.refreshTrack(updated);
    setSnapshot((current) => {
      if (!current) return current;
      const lovedDelta = baseline ? Number(updated.loved) - Number(baseline.loved) : 0;
      const ratedDelta = baseline ? Number(updated.rating !== null) - Number(baseline.rating !== null) : 0;
      return {
        ...current,
        summary: {
          ...current.summary,
          loved: Math.max(0, current.summary.loved + lovedDelta),
          rated: Math.max(0, current.summary.rated + ratedDelta),
        },
        tracks: current.tracks.map(refreshMatchingTrack),
      };
    });
  }

  shortcutResultHandlerRef.current = (result) => {
    const projection = result.success && result.catalogSync
      ? acceptTrackProjectionKeys(
        result.track ? [result.track.trackKey] : [],
        result.catalogSync.projectionToken,
      )
      : null;
    if (result.track && projection && !projection.acceptedTrackKeys.has(result.track.trackKey)) return;
    setSyncMessage(result.success ? result.message : `Shortcut failed: ${result.message}`);
    if (result.track) applyTrackChange(result.track, result.previousTrack ?? undefined);
    if (result.success && result.catalogSync) {
      void handleCatalogSync(result.catalogSync, true);
    }
  };

  async function saveGlobalShortcuts(request: GlobalShortcutSettingsRequest) {
    setShortcutSaving(true);
    setShortcutError(null);
    try {
      setShortcutStatus(await updateGlobalShortcutSettings(request));
    } catch (error) {
      setShortcutError(error instanceof Error ? error.message : String(error));
    } finally {
      setShortcutSaving(false);
    }
  }

  async function saveAudioSettings(request: AudioSettingsRequest) {
    setAudioSaving(true);
    setAudioError(null);
    try {
      setAudioStatus(await updateAudioSettings(request));
    } catch (error) {
      setAudioError(error instanceof Error ? error.message : String(error));
    } finally {
      setAudioSaving(false);
    }
  }

  function openSettings(tab: SettingsTab = "audio") {
    setSettingsInitialTab(tab);
    setShortcutError(null);
    setAudioError(null);
    setSettingsOpen(true);
  }

  async function saveInlineTagChange(track: Track, desired: TagValues) {
    if (inlineSaveRef.current.has(track.trackKey)) return;
    const expected = tagValuesForTrack(track);
    if (JSON.stringify(expected) === JSON.stringify(desired)) return;

    inlineSaveRef.current.add(track.trackKey);
    setInlineSavingKeys((current) => new Set(current).add(track.trackKey));
    setSyncMessage(null);

    const optimistic = trackWithTagValues(track, desired);
    const projectionTokenAtStart = latestTrackProjectionTokensRef.current.get(track.trackKey) ?? 0;
    applyTrackChange(optimistic, track, false);
    try {
      const snapshot = await updateTrackTags(track, expected, desired);
      const projection = acceptTrackProjectionKeys(
        [snapshot.track.trackKey],
        snapshot.catalogSync?.projectionToken,
      );
      if (!projection.acceptedTrackKeys.has(snapshot.track.trackKey)) return;
      applyTrackChange(snapshot.track, optimistic);
      await handleCatalogSync(snapshot.catalogSync, true);
      setInlineTagRevisions((current) => ({
        ...current,
        [track.trackKey]: (current[track.trackKey] ?? 0) + 1,
      }));
      if (snapshot.track.albumId && snapshot.track.albumId === selectedAlbumId) {
        const albumId = snapshot.track.albumId;
        const requestId = ++albumRequestRef.current;
        try {
          const detail = await loadAlbumDetail(albumId);
          if (requestId === albumRequestRef.current) {
            setExplorerAlbums((current) => current.map((album) => album.id === albumId ? detail.album : album));
            setAlbumTracks(detail.tracks);
            setAlbumTracksTruncated(detail.tracksTruncated);
            setSelectedTrack((current) => {
              if (!current || current.albumId !== albumId) return current;
              return detail.tracks.find((candidate) => candidate.trackKey === current.trackKey) ?? current;
            });
          }
        } catch (error) {
          console.warn("Aurora could not refresh the album rating after the track edit", error);
        }
      }
    } catch (error) {
      if ((latestTrackProjectionTokensRef.current.get(track.trackKey) ?? 0) === projectionTokenAtStart) {
        applyTrackChange(track, optimistic, false);
      }
      const message = error instanceof Error ? error.message : String(error);
      setSyncMessage(`Could not save ${track.title}: ${message}`);
    } finally {
      inlineSaveRef.current.delete(track.trackKey);
      setInlineSavingKeys((current) => {
        const next = new Set(current);
        next.delete(track.trackKey);
        return next;
      });
    }
  }

  function changeExplorerView(view: ExplorerView) {
    setExplorerView(view);
    setExplorerFilters((current) => ({
      ...current,
      sort: explorerSorts[view].includes(current.sort) ? current.sort : defaultExplorerSort[view],
    }));
    if (view !== "albums") setSelectedAlbumId(null);
  }

  function expandLibraryNavigation() {
    setLayoutPreferences((current) => current.libraryExpanded
      ? current
      : { ...current, libraryExpanded: true });
  }

  function navigate(label: SidebarDestination) {
    setActiveNav(label);
    if (label !== "Universe" && label !== "Observatory" && label !== "History") {
      expandLibraryNavigation();
    }
    if (label === "Observatory" || label === "History" || label === "Genres" || label === "Publishers" || label === "Years" || label === "Ratings") return;
    if (label === "Albums") changeExplorerView("albums");
    else if (label === "Artists") changeExplorerView("artists");
    else changeExplorerView("tracks");
  }

  function focusArtist(artist: Artist, destination: "tracks" | "albums" = "tracks") {
    setSelectedArtistId(artist.id);
    setActiveNav(destination === "albums" ? "Albums" : "Artists");
    expandLibraryNavigation();
    setExplorerView(destination);
    setExplorerFilters((current) => ({ ...current, artist: artist.name, sort: defaultExplorerSort[destination] }));
    if (destination === "albums") setSelectedAlbumId(null);
    openArtistInspector(artist.name);
  }

  function exploreArtistInLibrary(artistName: string) {
    setActiveNav("Artists");
    expandLibraryNavigation();
    setExplorerView("tracks");
    setExplorerFilters((current) => ({ ...current, artist: artistName, sort: "newest" }));
  }

  function exploreGenreInLibrary(genre: string) {
    setActiveNav("Songs");
    expandLibraryNavigation();
    setExplorerView("tracks");
    setExplorerFilters({ ...defaultExplorerFilters, genre, sort: "newest" });
  }

  function openArtistInspector(artistName: string) {
    const requestId = ++artistRequestRef.current;
    setInspectorArtistName(artistName);
    setInspectorView("artist");
    setArtistDetail(null);
    setArtistIntelligence(null);
    setArtistWorldError(null);
    setCurationError(null);
    setArtistWorldState("loading");
    void Promise.allSettled([
      loadArtistDetail(artistName),
      loadArtistIntelligence(artistName),
    ]).then(([catalogResult, intelligenceResult]) => {
      if (requestId !== artistRequestRef.current) return;
      if (catalogResult.status === "fulfilled") setArtistDetail(catalogResult.value);
      if (intelligenceResult.status === "fulfilled") setArtistIntelligence(intelligenceResult.value);
      if (catalogResult.status === "rejected" && intelligenceResult.status === "rejected") {
        const catalogMessage = catalogResult.reason instanceof Error ? catalogResult.reason.message : String(catalogResult.reason);
        const intelligenceMessage = intelligenceResult.reason instanceof Error ? intelligenceResult.reason.message : String(intelligenceResult.reason);
        setArtistWorldError(`${catalogMessage} ${intelligenceMessage}`);
        setArtistWorldState("error");
        return;
      }
      setArtistWorldError(catalogResult.status === "rejected"
        ? "The local catalog summary is unavailable; MusicBrainz context is still shown."
        : intelligenceResult.status === "rejected"
          ? "MusicBrainz context is unavailable; the local catalog remains usable."
          : null);
      setArtistWorldState("ready");
    });
  }

  openArtistInspectorRef.current = openArtistInspector;

  async function applyArtistDecision(request: ArtistDecisionRequest) {
    if (curationActionBusy) return;
    const requestId = ++artistRequestRef.current;
    setCurationActionBusy("artist");
    setCurationError(null);
    setCurationMessage(null);
    try {
      const intelligence = await updateArtistIdentityDecision(request);
      if (requestId !== artistRequestRef.current || inspectorArtistName !== request.artist) return;
      setArtistIntelligence(intelligence);
      setArtistWorldState("ready");
      setCurationMessage(request.action === "clear"
        ? `Cleared Aurora's identity override for ${request.artist}.`
        : request.action === "ignore"
          ? `Ignored ${request.artist} in Aurora.`
          : `Confirmed ${request.artist} as ${intelligence.identity?.canonicalName ?? request.artist}.`);
      setReviewReloadToken((value) => value + 1);
    } catch (error) {
      if (requestId === artistRequestRef.current) {
        setCurationError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      setCurationActionBusy((current) => current === "artist" ? null : current);
    }
  }

  async function applyReleaseDecision(request: ReleaseDecisionRequest) {
    if (curationActionBusy) return;
    const requestId = ++artistRequestRef.current;
    setCurationActionBusy(`release:${request.releaseMbid}`);
    setCurationError(null);
    setCurationMessage(null);
    try {
      const intelligence = await updateReleaseGroupDecision(request);
      if (requestId !== artistRequestRef.current || inspectorArtistName !== request.artist) return;
      setArtistIntelligence(intelligence);
      setArtistWorldState("ready");
      setCurationMessage(request.action === "link"
        ? "Linked the MusicBrainz release group to the selected local album."
        : request.action === "notInScope"
          ? "Marked the release group as not in scope."
          : request.action === "ignore"
            ? "Ignored the release group."
            : "Cleared Aurora's release override.");
      setReviewReloadToken((value) => value + 1);
    } catch (error) {
      if (requestId === artistRequestRef.current) {
        setCurationError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      setCurationActionBusy((current) => current === `release:${request.releaseMbid}` ? null : current);
    }
  }

  async function loadMoreReviewItems() {
    if (!reviewCursor || reviewLoadingMore) return;
    const requestId = ++reviewRequestRef.current;
    setReviewLoadingMore(true);
    try {
      const page = await loadArtistReviewPage({
        pageSize: 50,
        cursor: reviewCursor,
        filter: reviewFilter,
        search: reviewSearch.trim() || undefined,
      });
      if (requestId !== reviewRequestRef.current) return;
      setReviewItems((current) => {
        const existing = new Set(current.map((item) => item.artistKey));
        return [...current, ...page.items.filter((item) => !existing.has(item.artistKey))];
      });
      setReviewCursor(page.nextCursor);
    } catch (error) {
      if (requestId === reviewRequestRef.current) {
        setReviewError(error instanceof Error ? error.message : String(error));
        setReviewLoadState("error");
      }
    } finally {
      if (requestId === reviewRequestRef.current) setReviewLoadingMore(false);
    }
  }

  async function undoCuration() {
    if (curationActionBusy) return;
    setCurationActionBusy("undo");
    setCurationMessage(null);
    try {
      const intelligence = await undoMusicBrainzCuration();
      if (!intelligence) {
        setCurationMessage("There is no Aurora curation decision to undo.");
        return;
      }
      setInspectorArtistName(intelligence.artist);
      setInspectorView("artist");
      setArtistIntelligence(intelligence);
      setArtistWorldState("ready");
      setCurationError(null);
      setCurationMessage(`Undid the latest decision for ${intelligence.artist}.`);
      void loadArtistDetail(intelligence.artist).then(setArtistDetail).catch(() => setArtistDetail(null));
      setReviewReloadToken((value) => value + 1);
    } catch (error) {
      setCurationMessage(`Could not undo the latest decision: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setCurationActionBusy(null);
    }
  }

  async function exportCuration() {
    if (curationActionBusy) return;
    setCurationActionBusy("export");
    setCurationMessage(null);
    try {
      const result = await exportMusicBrainzCuration();
      setCurationMessage(`Exported ${formatCount(result.artistDecisions)} artist and ${formatCount(result.releaseDecisions)} release decisions to ${result.path}`);
    } catch (error) {
      setCurationMessage(`Could not export the overlay snapshot: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setCurationActionBusy(null);
    }
  }

  function selectAlbum(album: ExplorerAlbum | null) {
    const requestId = ++albumRequestRef.current;
    artistRequestRef.current += 1;
    setSelectedAlbumId(album?.id ?? null);
    setAlbumTracks([]);
    setAlbumTracksTruncated(false);
    if (!album) {
      setAlbumDetailState("ready");
      setInspectorView("track");
      setTagSelectionKind("track");
      return;
    }
    setSelectedTrack(null);
    setTagSelectionKind("album");
    if (inspectorViewRef.current !== "tags") setInspectorView("album");
    setAlbumDetailState("loading");
    void loadAlbumDetail(album.id)
      .then((detail) => {
        if (requestId !== albumRequestRef.current) return;
        setExplorerAlbums((current) => current.map((candidate) => candidate.id === detail.album.id ? detail.album : candidate));
        setAlbumTracks(detail.tracks);
        setAlbumTracksTruncated(detail.tracksTruncated);
        setSelectedTrack(detail.tracks[0] ?? null);
        setAlbumDetailState("ready");
      })
      .catch((error: unknown) => {
        if (requestId !== albumRequestRef.current) return;
        console.warn("Aurora could not open album details", error);
        setAlbumDetailState("error");
      });
  }

  async function playExplorerAlbum(album: ExplorerAlbum) {
    try {
      const tracks = selectedAlbumId === album.id && albumTracks.length > 0
        ? albumTracks
        : (await loadAlbumDetail(album.id)).tracks;
      if (tracks.length === 0) {
        setSyncMessage(`${album.title} has no playable tracks in the bounded album detail.`);
        return;
      }
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) selectTrack(tracks[0]);
    } catch (error) {
      setSyncMessage(`Could not play ${album.title}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  async function loadMoreExplorerResults() {
    if (!explorerCursor || isLoadingMore) return;
    const requestId = ++exploreRequestRef.current;
    setIsLoadingMore(true);
    try {
      const page = await loadExplorerPage(explorerView, explorerFilters, explorerCursor);
      if (requestId !== exploreRequestRef.current) return;
      setExplorerTracks((current) => [...current, ...page.tracks]);
      setExplorerAlbums((current) => [...current, ...page.albums]);
      setExplorerArtists((current) => [...current, ...page.artists]);
      setExplorerCursor(page.nextCursor);
      setExplorerCount({ key: explorerCountKey(explorerView, explorerFilters), total: page.totalCount });
    } catch (error) {
      if (requestId === exploreRequestRef.current) {
        setExplorerError(error instanceof Error ? error.message : String(error));
        setExplorerLoadState("error");
      }
    } finally {
      if (requestId === exploreRequestRef.current) setIsLoadingMore(false);
    }
  }

  async function loadMoreHistory() {
    if (!historyPage?.nextCursor || historyLoadingMore) return;
    const requestId = ++historyRequestRef.current;
    setHistoryLoadingMore(true);
    try {
      const startedAfterMs = historyDateRange === "all"
        ? undefined
        : Date.now() - Number(historyDateRange) * 86_400_000;
      const next = await loadHistoryPage({
        pageSize: 50,
        cursor: historyPage.nextCursor,
        search: historySearch.trim() || undefined,
        outcome: historyOutcome,
        deviceId: historyDeviceId ?? undefined,
        startedAfterMs,
      });
      if (requestId !== historyRequestRef.current) return;
      setHistoryPage((current) => current ? {
        ...next,
        items: [...current.items, ...next.items],
      } : next);
    } catch (error) {
      if (requestId === historyRequestRef.current) {
        setHistoryError(error instanceof Error ? error.message : String(error));
        setHistoryLoadState("error");
      }
    } finally {
      if (requestId === historyRequestRef.current) setHistoryLoadingMore(false);
    }
  }

  async function savePlayedThreshold(value: number) {
    setHistorySavingThreshold(true);
    setHistoryThresholdMessage(null);
    try {
      const saved = await saveHistoryPlayThreshold(value);
      setHistoryPage((current) => current ? { ...current, playThresholdSeconds: saved } : current);
      setHistoryThresholdMessage(`A play now registers after ${saved} ${saved === 1 ? "second" : "seconds"}.`);
    } catch (error) {
      setHistoryThresholdMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setHistorySavingThreshold(false);
    }
  }

  function playHistoryTrack(track: Track) {
    endGenreQueue();
    selectTrack(track);
    void playback.play([track], track.id);
  }

  function submitSearch(event: FormEvent) {
    event.preventDefault();
  }

  const explorerAlbumInspectorContext = activeNav === "Albums" && explorerView === "albums"
    ? resolveExplorerAlbumInspectorContext(explorerAlbums, selectedAlbumId, albumTracks, selectedTrack)
    : null;
  const inspectorTrack = explorerAlbumInspectorContext
    ? explorerAlbumInspectorContext.track
    : selectedTrack;
  const inspectorAlbumAvailable = Boolean(
    explorerAlbumInspectorContext
    || (activeNav === "Publishers" && selectedPublisherAlbum)
    || (activeNav === "Years" && selectedYearAlbum)
    || (activeNav === "Ratings" && selectedRatingAlbum)
    || (activeNav === "Charts" && chartSelection?.kind === "albums"),
  );
  const inspectorArtistCandidate = explorerAlbumInspectorContext?.artistName
    ?? (inspectorTrack ? displayTrackArtist(inspectorTrack) : null)
    ?? inspectorArtistName;
  const albumTagTarget = explorerAlbumInspectorContext
    ? { kind: "album" as const, albumId: explorerAlbumInspectorContext.album.id, label: explorerAlbumInspectorContext.album.title }
    : activeNav === "Publishers" && selectedPublisherAlbum
      ? { kind: "album" as const, albumId: selectedPublisherAlbum.id, label: selectedPublisherAlbum.title }
      : activeNav === "Years" && selectedYearAlbum
        ? { kind: "album" as const, albumId: selectedYearAlbum.id, label: selectedYearAlbum.title }
        : activeNav === "Ratings" && selectedRatingAlbum
          ? { kind: "album" as const, albumId: selectedRatingAlbum.id, label: selectedRatingAlbum.title }
          : activeNav === "Charts" && chartSelection?.kind === "albums" && chartSelection.entry.matchedAlbumId
            ? { kind: "album" as const, albumId: chartSelection.entry.matchedAlbumId, label: chartSelection.entry.title }
            : null;
  const tagEditorTarget = tagSelectionKind === "album"
    ? albumTagTarget
    : inspectorTrack
      ? { kind: "track" as const, trackId: inspectorTrack.id, trackKey: inspectorTrack.trackKey, label: inspectorTrack.title }
      : null;

  const explorerLoaded = explorerView === "tracks"
    ? explorerTracks.length
    : explorerView === "albums"
      ? explorerAlbums.length
      : explorerArtists.length;
  const currentExplorerCount = explorerCount?.key === explorerCountKey(explorerView, explorerFilters)
    ? explorerCount.total
    : null;
  const explorerCountNoun = explorerView === "tracks"
    ? ["song", "songs"] as const
    : explorerView === "albums"
      ? ["album", "albums"] as const
      : ["artist", "artists"] as const;
  const showExplorerCount = snapshot !== null
    && !["Observatory", "Charts", "History", "Genres", "Publishers", "Years", "Ratings"].includes(activeNav);
  const topbarSearchValue = activeNav === "Observatory"
    ? reviewSearch
    : activeNav === "History"
      ? historySearch
      : activeNav === "Genres"
        ? genreSearch
        : activeNav === "Publishers"
          ? publisherSearch
        : activeNav === "Years"
          ? ""
        : explorerFilters.query;
  const topbarSearchPlaceholder = activeNav === "Observatory"
    ? "Search artists to review…"
    : activeNav === "History"
      ? "Search listening history…"
      : activeNav === "Genres"
        ? "Search your genre atlas…"
        : activeNav === "Publishers"
          ? "Search publishers…"
        : activeNav === "Years"
          ? "Year search arrives with the timeline…"
        : explorerView === "tracks"
          ? "Search year:1985..1987, OR, NOT…"
          : "Search your universe…";
  const topbarSearchLabel = activeNav === "Observatory"
    ? "Search MusicBrainz review artists"
    : activeNav === "History"
      ? "Search listening history"
      : activeNav === "Genres"
        ? "Search genres"
        : activeNav === "Publishers"
          ? "Search publishers"
        : activeNav === "Years"
          ? "Year search is not available yet"
        : "Search your music universe";

  function updateTopbarSearch(value: string) {
    if (activeNav === "Observatory") setReviewSearch(value);
    else if (activeNav === "History") setHistorySearch(value);
    else if (activeNav === "Genres") setGenreSearch(value);
    else if (activeNav === "Publishers") setPublisherSearch(value);
    else if (activeNav === "Years") return;
    else setExplorerFilters((current) => ({ ...current, query: value }));
  }

  const summary = snapshot?.summary;
  const stats = [
    { label: "Songs", value: summary?.songs, detail: "indexed", icon: Music2, tone: "violet" },
    { label: "Albums", value: summary?.albums, detail: "releases", icon: Album, tone: "cyan" },
    { label: "Artists", value: summary?.artists, detail: "worlds", icon: UsersRound, tone: "amber" },
    { label: "Loved", value: summary?.loved, detail: "favorites", icon: Heart, tone: "rose" },
  ];
  const leftSidebarAction = layoutPreferences.leftSidebar === "expanded"
    ? "Switch left sidebar to icon-only mode"
    : layoutPreferences.leftSidebar === "icons"
      ? "Collapse left sidebar"
      : "Expand left sidebar";
  const LeftSidebarIcon = layoutPreferences.leftSidebar === "expanded"
    ? PanelLeft
    : layoutPreferences.leftSidebar === "icons"
      ? PanelLeftClose
      : PanelLeftOpen;
  const rightSidebarAction = layoutPreferences.rightSidebar === "expanded"
    ? "Collapse right sidebar"
    : "Expand right sidebar";
  const RightSidebarIcon = layoutPreferences.rightSidebar === "expanded"
    ? PanelRightClose
    : PanelRightOpen;
  const activeDisplayView = displayViewByDestination[activeNav];
  const activeDisplayPreferences = effectiveDisplayPreferences(displayPreferences, activeDisplayView);
  const catalogNoticeMessage = catalogSyncNotice ? catalogSyncMessage(catalogSyncNotice) : null;

  return (
    <div
      className="app-shell"
      data-left-sidebar={layoutPreferences.leftSidebar}
      data-right-sidebar={layoutPreferences.rightSidebar}
      data-text-size={displayPreferences.global.textSize}
      data-cover-size={displayPreferences.global.coverSize}
    >
      {layoutPreferences.leftSidebar !== "collapsed" && <aside className="sidebar">
        <div className="brand">
          <div className="brand__mark"><AudioLines aria-hidden="true" /></div>
          <div><strong>Aurora</strong><span>your music, your universe</span></div>
        </div>

        <p className="sidebar__label">Navigation</p>
        <SidebarNavigation
          key={layoutPreferences.leftSidebar}
          activeDestination={activeNav}
          sidebarMode={layoutPreferences.leftSidebar}
          libraryExpanded={layoutPreferences.libraryExpanded}
          playlistsExpanded={layoutPreferences.playlistsExpanded}
          onLibraryExpandedChange={(libraryExpanded) => setLayoutPreferences((current) => ({
            ...current,
            libraryExpanded,
          }))}
          onPlaylistsExpandedChange={(playlistsExpanded) => setLayoutPreferences((current) => ({
            ...current,
            playlistsExpanded,
          }))}
          onNavigate={navigate}
        />

        <div className="profile">
          <CircleUserRound aria-hidden="true" />
          <span><strong>Jørn</strong><small>Aurora 0.17.11</small></span>
          <Settings aria-hidden="true" />
        </div>
      </aside>}

      <header className="topbar">
        <div className="topbar__primary">
          <button
            type="button"
            className="layout-toggle"
            data-mode={layoutPreferences.leftSidebar}
            aria-label={leftSidebarAction}
            title={`${leftSidebarAction}. Current mode: ${layoutPreferences.leftSidebar}.`}
            onClick={() => setLayoutPreferences((current) => ({
              ...current,
              leftSidebar: nextLeftSidebarMode(current.leftSidebar),
            }))}
          >
            <LeftSidebarIcon aria-hidden="true" />
          </button>
          <form className={`search${activeNav === "Years" ? " is-disabled" : ""}`} role="search" onSubmit={submitSearch}>
            <Search aria-hidden="true" />
            <input
              ref={searchRef}
              value={topbarSearchValue}
              onChange={(event) => updateTopbarSearch(event.target.value)}
              placeholder={topbarSearchPlaceholder}
              aria-label={topbarSearchLabel}
              title={explorerView === "tracks" && !["Observatory", "History", "Genres", "Publishers", "Years"].includes(activeNav) ? trackSearchHelp : undefined}
              disabled={activeNav === "Years"}
            />
            {topbarSearchValue
              ? <button type="button" aria-label="Clear search" onClick={() => updateTopbarSearch("")}><X aria-hidden="true" /></button>
              : activeNav !== "Years" ? <kbd>Ctrl K</kbd> : null}
          </form>
          {showExplorerCount ? (
            <output className="search-result-count" aria-live="polite" aria-busy={currentExplorerCount === null}>
              {currentExplorerCount === null ? (
                <>Counting {explorerCountNoun[1]}…</>
              ) : (
                <><strong>{formatCount(currentExplorerCount)}</strong> {currentExplorerCount === 1 ? explorerCountNoun[0] : explorerCountNoun[1]}</>
              )}
            </output>
          ) : null}
        </div>
        <div className="topbar__actions">
          <button type="button" className="add-music-action" onClick={() => setAddFolderOpen(true)}><FolderPlus aria-hidden="true" /><span>Add music</span></button>
          {syncMessage && <span className="tag-sync-message" role="status">{syncMessage}</span>}
          {catalogNoticeMessage && (
            <span
              className="tag-sync-message"
              data-sync-status={catalogSyncNotice?.status}
              role="status"
              title={catalogNoticeMessage}
            >{catalogNoticeMessage}</span>
          )}
          <LaptopModeButton
            status={laptopModeStatus}
            busy={laptopModeBusy}
            error={laptopModeError}
            onToggle={() => void toggleLaptopMode()}
          />
          <button type="button" aria-label="Audio settings" title="Audio settings" onClick={() => openSettings("audio")}><AudioLines aria-hidden="true" /></button>
          <button type="button" aria-label="Labs" disabled><FlaskConical aria-hidden="true" /></button>
          {updater.state.version && <button type="button" className="update-badge" onClick={updater.showPrompt}><Download aria-hidden="true" /> Update {updater.state.version}</button>}
          <button
            type="button"
            className="layout-toggle"
            data-mode={layoutPreferences.rightSidebar}
            aria-label={rightSidebarAction}
            title={`${rightSidebarAction}. Current mode: ${layoutPreferences.rightSidebar}.`}
            onClick={() => setLayoutPreferences((current) => ({
              ...current,
              rightSidebar: current.rightSidebar === "expanded" ? "collapsed" : "expanded",
            }))}
          >
            <RightSidebarIcon aria-hidden="true" />
          </button>
          <button type="button" aria-label="Settings" title="Settings" onClick={() => openSettings("display")}><Settings aria-hidden="true" /></button>
        </div>
      </header>

      <main className="main-content">
        <div
          className="main-scroll"
          data-text-size={activeDisplayPreferences.textSize}
          data-cover-size={activeDisplayPreferences.coverSize}
        >
          {snapshot ? (
            activeNav === "Observatory" ? (
              <Observatory
                items={reviewItems}
                selectedArtistKey={artistIntelligence?.artistKey ?? null}
                filter={reviewFilter}
                loadState={reviewLoadState}
                errorMessage={reviewError}
                hasMore={reviewCursor !== null}
                loadingMore={reviewLoadingMore}
                actionBusy={curationActionBusy === "export" || curationActionBusy === "undo" ? curationActionBusy : null}
                message={curationMessage}
                onFilterChange={setReviewFilter}
                onSelect={(item) => openArtistInspector(item.displayArtist)}
                onLoadMore={() => void loadMoreReviewItems()}
                onRefresh={() => setReviewReloadToken((value) => value + 1)}
                onUndo={() => void undoCuration()}
                onExport={() => void exportCuration()}
              />
            ) : activeNav === "Charts" ? (
              <ChartStudio
                catalogRevision={chartReloadToken}
                onSelectionChange={(selection, options) => {
                  setChartSelection(selection);
                  if (selection && !options?.preserveInspector) {
                    setTagSelectionKind(selection.kind === "albums" ? "album" : "track");
                    setInspectorView(selection.kind === "albums" ? "album" : "track");
                  }
                }}
                onSelectTrack={(track, options) => {
                  if (!options?.preserveInspector) {
                    selectTrack(track);
                    return;
                  }
                  setSelectedTrack((current) => current?.trackKey === track.trackKey ? track : current);
                }}
                onPlayQueue={playChartQueue}
              />
            ) : activeNav === "History" ? (
              <ListeningHistory
                page={historyPage}
                loadState={historyLoadState}
                errorMessage={historyError}
                search={historySearch}
                outcome={historyOutcome}
                deviceId={historyDeviceId}
                dateRange={historyDateRange}
                isLoadingMore={historyLoadingMore}
                isSavingThreshold={historySavingThreshold}
                thresholdMessage={historyThresholdMessage}
                onSearchChange={setHistorySearch}
                onOutcomeChange={setHistoryOutcome}
                onDeviceChange={setHistoryDeviceId}
                onDateRangeChange={setHistoryDateRange}
                onSaveThreshold={(value) => void savePlayedThreshold(value)}
                onSelectTrack={selectTrack}
                onPlayTrack={playHistoryTrack}
                onLoadMore={() => void loadMoreHistory()}
                onRefresh={() => setHistoryReloadToken((value) => value + 1)}
              />
            ) : activeNav === "Genres" ? (
              <GenreAtlas
                genres={genreAtlasGenres}
                selectedGenre={selectedGenre}
                detail={genreDetail}
                search={genreSearch}
                indexState={genreIndexState}
                detailState={genreDetailState}
                indexError={genreIndexError}
                detailError={genreDetailError}
                queueBusy={genreQueueBusy}
                queueMessage={genreQueueMessage}
                radioSession={genreRadioSession}
                busyTrackKeys={inlineSavingKeys}
                onSearchChange={setGenreSearch}
                onSelectGenre={setSelectedGenre}
                onRetryIndex={() => setGenreIndexReloadToken((value) => value + 1)}
                onRetryDetail={() => setGenreDetailReloadToken((value) => value + 1)}
                onQueue={(mode) => void startGenreQueue(mode)}
                onOpenTracks={exploreGenreInLibrary}
                onOpenArtist={(artist) => {
                  exploreArtistInLibrary(artist);
                  openArtistInspector(artist);
                }}
                onSelectTrack={selectTrack}
                onPlayTrack={(track) => playTrack(track, genreDetail?.highlights ?? [track])}
                onRatingChange={(track, rating) => void saveInlineTagChange(track, { ...tagValuesForTrack(track), rating })}
                onLoveChange={(track, loveState) => void saveInlineTagChange(track, { ...tagValuesForTrack(track), loveState })}
              />
            ) : activeNav === "Publishers" ? (
              <PublisherSignalTimeline
                overview={publisherOverview}
                detail={publisherDetail}
                loadState={publisherLoadState}
                detailState={publisherDetailState}
                errorMessage={publisherError}
                detailError={publisherDetailError}
                selectedAlbumId={selectedPublisherAlbum?.id ?? null}
                queueBusy={publisherQueueBusy}
                queueMessage={publisherQueueMessage}
                onSelectPublisher={selectPublisher}
                onSelectAlbum={openPublisherAlbum}
                onExplore={explorePublisher}
                onPlayPublisher={(publisher) => void playPublisher(publisher)}
                onRetry={() => setPublisherReloadToken((value) => value + 1)}
                onRetryDetail={() => publisherDetail && selectPublisher(publisherDetail.publisher)}
              />
            ) : activeNav === "Years" ? (
              <YearsExplorer
                overview={yearOverview}
                detail={yearDetail}
                loadState={yearLoadState}
                detailState={yearDetailState}
                errorMessage={yearError}
                detailError={yearDetailError}
                selectedAlbumId={selectedYearAlbum?.id ?? null}
                queueBusy={yearQueueBusy}
                queueMessage={yearQueueMessage}
                onSelect={selectYear}
                onSelectAlbum={openYearAlbum}
                onExplore={exploreYear}
                onPlayYear={(selection) => void playYear(selection)}
                onPlayAlbum={(album) => void playYearAlbum(album)}
                onRetry={() => setYearReloadToken((value) => value + 1)}
                onRetryDetail={() => yearDetail && selectYear(yearDetail.selection)}
              />
            ) : activeNav === "Ratings" ? (
              <RatingsStudio
                overview={ratingsOverview}
                page={ratingsPage}
                selectedAlbum={selectedRatingAlbum}
                albumTracks={ratingAlbumTracks}
                loadState={ratingsLoadState}
                pageState={ratingsPageState}
                errorMessage={ratingsError}
                pageError={ratingsPageError}
                queueBusy={ratingsQueueBusy}
                queueMessage={ratingsQueueMessage}
                busyTrackKeys={inlineSavingKeys}
                onCompletionChange={setRatingsCompletion}
                onSelectAlbum={openRatingAlbum}
                onSelectTrack={selectTrack}
                onPlayTrack={(track) => playTrack(track, ratingAlbumTracks)}
                onRatingChange={(track, rating) => void saveInlineTagChange(track, { ...tagValuesForTrack(track), rating })}
                onLoveChange={(track, loveState) => void saveInlineTagChange(track, { ...tagValuesForTrack(track), loveState })}
                onPlayCollection={(mode, rating) => void playRatingCollection(mode, rating)}
                onExploreCollection={exploreRatingCollection}
                onPlayUnrated={(album) => void playRatingAlbumUnrated(album)}
                onRefresh={() => setRatingsReloadToken((value) => value + 1)}
                onRetry={() => setRatingsReloadToken((value) => value + 1)}
                onRetryPage={() => setRatingsReloadToken((value) => value + 1)}
              />
            ) : <>
              {activeNav === "Universe" ? <>
              <Universe artists={snapshot.artists} activeArtist={explorerFilters.artist} onSelect={focusArtist} />
              <section className="stats" aria-label="Library overview">
                {stats.map(({ label, value, detail, icon: Icon, tone }) => (
                  <article className={`stat stat--${tone}`} key={label}>
                    <div className="stat__icon"><Icon aria-hidden="true" /></div>
                    <div><span>{label}</span><strong>{formatCount(value ?? 0)}</strong><small>{detail}</small></div>
                    <Activity className="stat__spark" aria-hidden="true" />
                  </article>
                ))}
                <article className="source-card">
                  <Gauge aria-hidden="true" />
                  <div><span>Source</span><strong>{snapshot.sourceState === "connected" ? "Live" : "Preview"}</strong><small>{snapshot.sourceLabel}</small></div>
                  {snapshot.sourceState === "connected" && <BadgeCheck aria-label="Connected read-only" />}
                </article>
              </section>
              {historyPage && (
                <section className="memory-strip" aria-label="Listening memory">
                  <div className="memory-strip__heading"><Clock3 aria-hidden="true" /><span><strong>Listening Memory</strong><small>{formatCount(historyPage.summary.plays)} registered plays across {historyPage.devices.length || 1} {historyPage.devices.length === 1 ? "device" : "devices"}</small></span></div>
                  {historyPage.items[0] ? <div className="memory-strip__recent"><span>Last heard</span><strong>{historyPage.items[0].title}</strong><small>{historyPage.items[0].artist}</small></div> : <div className="memory-strip__recent"><span>Ready to remember</span><strong>Play something you love</strong><small>It counts after {historyPage.playThresholdSeconds} seconds</small></div>}
                  <button type="button" onClick={() => navigate("History")}>Open History <ChevronRight aria-hidden="true" /></button>
                </section>
              )}
              </> : null}

              <DeepExplorer
                view={explorerView}
                filters={explorerFilters}
                tracks={explorerTracks}
                albums={explorerAlbums}
                artists={explorerArtists}
                selectedTrackId={selectedTrack?.id ?? null}
                currentTrackKey={playback.state.currentTrack?.trackKey ?? null}
                playbackActive={playback.state.status === "playing"}
                selectedAlbumId={selectedAlbumId}
                selectedArtistId={selectedArtistId}
                albumTracks={albumTracks}
                albumTracksTruncated={albumTracksTruncated}
                loadState={explorerLoadState}
                errorMessage={explorerError}
                albumDetailState={albumDetailState}
                pageInfo={{ loaded: explorerLoaded, hasMore: explorerCursor !== null, isLoadingMore }}
                busyTrackKeys={inlineSavingKeys}
                onViewChange={changeExplorerView}
                onFiltersChange={setExplorerFilters}
                onSelectTrack={selectTrack}
                onActivateTrack={(track) => playTrack(track, albumTracks.some((candidate) => candidate.id === track.id) ? albumTracks : explorerTracks)}
                onSelectAlbum={selectAlbum}
                onSelectArtist={(artist) => { if (artist) focusArtist(artist, "albums"); else setSelectedArtistId(null); }}
                onLoadMore={() => void loadMoreExplorerResults()}
                onRetry={() => {
                  if (selectedAlbumId && albumDetailState === "error") {
                    const album = explorerAlbums.find((candidate) => candidate.id === selectedAlbumId);
                    if (album) selectAlbum(album);
                  } else {
                    setExplorerReloadToken((value) => value + 1);
                  }
                }}
                onClearFilters={() => setExplorerFilters({ ...defaultExplorerFilters, sort: defaultExplorerSort[explorerView] })}
                onRatingChange={(track, rating) => void saveInlineTagChange(track, { ...tagValuesForTrack(track), rating })}
                onLoveChange={(track, loveState) => void saveInlineTagChange(track, { ...tagValuesForTrack(track), loveState })}
              />
            </>
          ) : loadError ? (
            <section className="load-state load-state--error" role="alert">
              <Disc3 aria-hidden="true" /><p className="eyebrow">Library unavailable</p><h1>Aurora kept your database untouched.</h1><p>{loadError}</p>
              <button type="button" className="button button--primary" onClick={() => { setLoadError(null); setReloadToken((value) => value + 1); }}><RefreshCw aria-hidden="true" /> Try again</button>
            </section>
          ) : (
            <section className="load-state" aria-live="polite"><div className="loading-orbit"><Disc3 aria-hidden="true" /></div><p>Opening your music universe read-only…</p></section>
          )}
        </div>
      </main>

      {layoutPreferences.rightSidebar === "expanded" && <aside
        className="inspector"
        data-text-size={activeDisplayPreferences.textSize}
        data-cover-size={activeDisplayPreferences.coverSize}
      >
        <div className="inspector-tabs" role="tablist" aria-label="Library details">
          <button type="button" role="tab" aria-selected={inspectorView === "track"} disabled={!inspectorTrack} onClick={() => setInspectorView("track")}>Track</button>
          <button type="button" role="tab" aria-selected={inspectorView === "album"} disabled={!inspectorAlbumAvailable} onClick={() => setInspectorView("album")}>Album</button>
          <button
            type="button"
            role="tab"
            aria-selected={inspectorView === "artist"}
            disabled={!inspectorArtistCandidate}
            onClick={() => {
              const artistName = explorerAlbumInspectorContext?.artistName
                ?? (inspectorTrack ? displayTrackArtist(inspectorTrack) : null)
                ?? inspectorArtistName;
              if (artistName) openArtistInspector(artistName);
            }}
          >Artist</button>
          <button type="button" role="tab" aria-selected={inspectorView === "tags"} disabled={!tagEditorTarget} onClick={() => setInspectorView("tags")}>Tags</button>
        </div>
        {inspectorView === "tags" && tagEditorTarget ? (
          <div className="inspector-scroll inspector-scroll--tag-editor">
            <TagEditor
              key={tagEditorTarget.kind === "album"
                ? `album:${tagEditorTarget.albumId}`
                : `track:${tagEditorTarget.trackKey}:${inlineTagRevisions[tagEditorTarget.trackKey] ?? 0}`}
              target={tagEditorTarget}
              onTracksChange={applyTrackChanges}
              onCatalogSync={(sync) => handleCatalogSync(sync, true)}
            />
          </div>
        ) : activeNav === "Charts" && chartSelection && ((chartSelection.kind === "singles" && inspectorView === "track") || (chartSelection.kind === "albums" && inspectorView === "album")) ? (
          <div className="inspector-scroll">
            <ChartInspector
              selection={chartSelection}
              track={chartSelection.kind === "singles" && selectedTrack?.id === chartSelection.entry.matchedTrackId ? selectedTrack : null}
              busy={chartPlaybackBusy || Boolean(selectedTrack && inlineSavingKeys.has(selectedTrack.trackKey))}
              onPlay={() => void playChartSelection()}
              onOpenLibrary={openChartSelectionInLibrary}
              onRatingChange={(track, rating) => void saveInlineTagChange(track, { ...tagValuesForTrack(track), rating })}
              onLoveChange={(track, loveState) => void saveInlineTagChange(track, { ...tagValuesForTrack(track), loveState })}
            />
          </div>
        ) : inspectorView === "album" && explorerAlbumInspectorContext ? (
          <div className="inspector-scroll">
            <YearAlbumInspector
              album={explorerAlbumInspectorContext.album}
              busy={albumDetailState === "loading"}
              onPlay={(album) => void playExplorerAlbum(album)}
            />
          </div>
        ) : inspectorView === "album" && activeNav === "Publishers" && selectedPublisherAlbum ? (
          <div className="inspector-scroll">
            <PublisherAlbumInspector album={selectedPublisherAlbum} busy={publisherAlbumBusy} onPlay={(album) => void playPublisherAlbum(album)} />
          </div>
        ) : inspectorView === "album" && activeNav === "Ratings" && selectedRatingAlbum ? (
          <div className="inspector-scroll">
            <RatingAlbumInspector album={selectedRatingAlbum} busy={ratingsQueueBusy} onPlay={(album) => void playRatingAlbumUnrated(album)} />
          </div>
        ) : inspectorView === "album" && activeNav === "Years" && selectedYearAlbum ? (
          <div className="inspector-scroll">
            <YearAlbumInspector album={selectedYearAlbum} busy={yearAlbumBusy} onPlay={(album) => void playYearAlbum(album)} />
          </div>
        ) : inspectorView === "artist" && inspectorArtistName ? (
          <div className="inspector-scroll">
            <ArtistWorld
              key={inspectorArtistName}
              artistName={inspectorArtistName}
              catalogDetail={artistDetail}
              intelligence={artistIntelligence}
              state={artistWorldState}
              errorMessage={artistWorldError}
              curationError={curationError}
              actionBusy={curationActionBusy}
              onRetry={() => openArtistInspector(inspectorArtistName)}
              onExploreLibrary={() => exploreArtistInLibrary(inspectorArtistName)}
              onArtistDecision={(request) => void applyArtistDecision(request)}
              onReleaseDecision={(request) => void applyReleaseDecision(request)}
            />
          </div>
        ) : inspectorTrack ? (
          <div className="inspector-scroll">
            <Artwork track={inspectorTrack} size="large" />
            <div className="track-hero-copy">
              <div><h2>{inspectorTrack.title}</h2><p>{displayTrackArtist(inspectorTrack)}</p><span>{inspectorTrack.album}</span></div>
              <button type="button" className="inspector-play" onClick={() => playTrack(inspectorTrack)}><Play aria-hidden="true" /> Play</button>
            </div>
            <dl className="metadata-list">
              <div className="publisher-metadata"><dt>Publisher</dt><dd>{inspectorTrack.publisher ?? "Unknown"}</dd></div>
              <div><dt>Genre</dt><dd>{inspectorTrack.genre ?? "Unknown"}</dd></div>
              <div><dt>Last.fm popularity</dt><dd>{inspectorTrack.playCount === null ? "—" : formatCount(inspectorTrack.playCount)}</dd></div>
              <div><dt>Duration</dt><dd>{formatDuration(inspectorTrack.durationSeconds)}</dd></div>
              <div><dt>Your registered plays</dt><dd>{trackHistory?.trackKey === inspectorTrack.trackKey ? formatCount(trackHistory.value.plays) : "—"}</dd></div>
              <div><dt>Your listening time</dt><dd>{trackHistory?.trackKey === inspectorTrack.trackKey ? formatDuration(Math.round(trackHistory.value.listenedSeconds)) : "—"}</dd></div>
              <div><dt>Last listened</dt><dd>{trackHistory?.trackKey === inspectorTrack.trackKey ? historyDateLabel(trackHistory.value.lastListenedAtMs) : "—"}</dd></div>
            </dl>
            <div className="readonly-note"><BadgeCheck aria-hidden="true" /><span><strong>Verified file writes</strong>Use the Tags tab to edit this MP3 or the selected album without leaving Aurora.</span></div>
          </div>
        ) : <EmptyInspector />}
      </aside>}

      {queueOpen && (
        <QueuePanel
          playback={playback.state}
          onClose={() => setQueueOpen(false)}
          onPlay={(trackId) => void playback.play(playback.state.queue, trackId)}
          onMove={(from, to) => void playback.move(from, to)}
          onRemove={(index) => void playback.remove(index)}
          onClear={() => {
            endGenreQueue();
            void playback.clear();
          }}
        />
      )}

      <PlayerBar
        playback={playback.state}
        isWorking={playback.isWorking}
        tagBusy={playback.state.currentTrack
          ? inlineSavingKeys.has(playback.state.currentTrack.trackKey)
          : false}
        error={playback.error}
        queueOpen={queueOpen}
        onDismissError={playback.dismissError}
        onToggle={() => void playback.toggle()}
        onPrevious={() => void playback.previous()}
        onNext={() => void playback.next()}
        onSeek={(position) => playback.seek(position)}
        onVolume={(volume) => playback.setVolume(volume)}
        onShuffle={(enabled) => void playback.setShuffle(enabled)}
        onRepeat={(mode) => void playback.setRepeatMode(mode)}
        onRatingChange={(track, rating) => void saveInlineTagChange(track, {
          ...tagValuesForTrack(track),
          rating,
        })}
        onLoveChange={(track, loveState) => void saveInlineTagChange(track, {
          ...tagValuesForTrack(track),
          loveState,
        })}
        onOpenAudioSettings={() => openSettings("audio")}
        onToggleQueue={() => setQueueOpen((open) => !open)}
      />

      {addFolderOpen && (
        <Suspense fallback={<div className="modal-backdrop"><div className="settings-loading" role="status">Opening album intake…</div></div>}>
          <AddFolderDialog
            onClose={() => setAddFolderOpen(false)}
            onCatalogChanged={refreshCatalogIfChanged}
          />
        </Suspense>
      )}
      {updater.state.isPromptOpen && <UpdateDialog version={updater.state.version} phase={updater.state.phase} progress={updater.state.progress} message={updater.state.message} onInstall={() => void updater.install()} onDismiss={updater.dismiss} />}
      {settingsOpen && shortcutStatus && audioStatus && (
        <SettingsDialog
          shortcutStatus={shortcutStatus}
          audioStatus={audioStatus}
          shortcutSaving={shortcutSaving}
          audioSaving={audioSaving}
          shortcutError={shortcutError}
          audioError={audioError}
          displayPreferences={displayPreferences}
          activeDisplayView={activeDisplayView}
          initialTab={settingsInitialTab}
          onSaveDisplay={setDisplayPreferences}
          onSaveShortcuts={(request) => void saveGlobalShortcuts(request)}
          onSaveAudio={(request) => void saveAudioSettings(request)}
          onClose={() => setSettingsOpen(false)}
        />
      )}
      {settingsOpen && (!shortcutStatus || !audioStatus) && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => {
          if (event.target === event.currentTarget) setSettingsOpen(false);
        }}>
          <div className="settings-loading" role="status">
            <span>{shortcutError ?? audioError ?? "Loading settings…"}</span>
            {(shortcutError || audioError) && <button type="button" aria-label="Close settings" onClick={() => setSettingsOpen(false)}><X aria-hidden="true" /></button>}
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
