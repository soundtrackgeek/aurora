import {
  Activity,
  Album,
  ArrowLeft,
  AudioLines,
  BadgeCheck,
  CircleUserRound,
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
import { lazy, Activity as ReactActivity, Suspense, useCallback, useEffect, useLayoutEffect, useRef, useState, type FormEvent } from "react";
import "./App.css";
import { AppStoreProvider } from "./app/AppStoreProvider";
import { useArtistNavigation } from "./app/artist/useArtistNavigation";
import { EmptyInspector } from "./app/components/EmptyInspector";
import { Universe } from "./app/components/Universe";
import { UpdateDialog } from "./app/components/UpdateDialog";
import { useGenresDomain } from "./app/domains/useGenresDomain";
import { useHistoryDomain } from "./app/domains/useHistoryDomain";
import { useObservatoryDomain } from "./app/domains/useObservatoryDomain";
import { usePlaylistsDomain } from "./app/domains/usePlaylistsDomain";
import { usePublishersDomain } from "./app/domains/usePublishersDomain";
import { useRatingsDomain } from "./app/domains/useRatingsDomain";
import { useYearsDomain } from "./app/domains/useYearsDomain";
import { explorerCountKey, explorerRequestKey } from "./app/explorer/explorerQueries";
import { useExplorerResults } from "./app/explorer/useExplorerResults";
import { useExplorerWorkspace } from "./app/explorer/useExplorerWorkspace";
import { useInspectorMetadata } from "./app/inspector/useInspectorMetadata";
import { useInspectorSelection } from "./app/inspector/useInspectorSelection";
import { useNavigationState, useWorkspaceNavigation } from "./app/navigation/useWorkspaceNavigation";
import { useWorkspaceCheckpoint, useWorkspaceRestoration } from "./app/navigation/useWorkspaceRestoration";
import {
  ArtistInspector,
  ArtistRoute,
  ChartInspector,
  ChartsRoute,
  GenresRoute,
  HistoryRoute,
  LibraryRoute,
  ObservatoryRoute,
  PlaylistsRoute,
  PublisherAlbumInspector,
  PublishersRoute,
  RatingAlbumInspector,
  RatingsRoute,
  YearAlbumInspector,
  YearsRoute
} from "./app/routes/DestinationRoutes";
import { useAppSettings } from "./app/settings/useAppSettings";
import { useTagMutations } from "./app/tags/useTagMutations";
import { useAppSlice, useAppStore } from "./app/useAppStore";
import { ArtistSmartLink } from "./components/ArtistSmartLink";
import { Artwork } from "./components/Artwork";
import { CatalogChartRanks } from "./components/charts/CatalogChartRanks";
import type { ChartSelectionContext } from "./components/charts/ChartStudio";
import { TrackChartInfo } from "./components/charts/TrackChartInfo";
import { ContentTransition } from "./components/ContentTransition";
import type { AlbumMoveRequest } from "./components/explorer/AlbumMoveOperation";
import { AlbumMoveOperation, SettingsDialog, TagEditor } from "./app/components/LazyPanels";
import {
  type ExplorerFilters,
  type ExplorerView
} from "./components/explorer/DeepExplorer";
import { resolveExplorerAlbumInspectorContext } from "./components/explorer/inspectorContext";
import { RemoveAlbumButton } from "./components/explorer/RemoveAlbumButton";
import { UniverseListeningMemory } from "./components/history/UniverseListeningMemory";
import { LaptopModeButton } from "./components/LaptopModeButton";
import { RememberedPage } from "./components/navigation/RememberedPage";
import {
  SidebarNavigation,
  type SidebarDestination,
} from "./components/navigation/SidebarNavigation";
import { PlayerBar } from "./components/PlayerBar";
import { QueuePanel } from "./components/QueuePanel";
import { transitionContent } from "./contentTransition";
import {
  effectiveDisplayPreferences,
  loadDisplayPreferences,
  saveDisplayPreferences,
  type DisplayViewKey,
} from "./displayPreferences";
import {
  loadLayoutPreferences,
  nextLeftSidebarMode,
  saveLayoutPreferences,
} from "./layoutPreferences";
import {
  applyAlbumTrackMetricsProjection,
  deleteAlbumTracks,
  displayTrackArtist,
  formatCount,
  formatDuration,
  loadAlbumDetail,
  loadLibrarySnapshot,
  type Artist,
  type LibrarySnapshot,
  type Track
} from "./library";
import { usePlayback } from "./playback";
import {
  type RatingAlbum,
  type RatingMode
} from "./ratings";
import { preparePopulatedInputForFocus } from "./searchFocusGuard";
import {
  tagValuesForTrack,
  trackWithReconciledTags,
  type TagReconciliationChange
} from "./tags";
import { useAuroraUpdater } from "./updater";
import {
  defaultExplorerFilters,
  defaultExplorerSort,
  explorerSorts,
  explorerViewForDestination,
  loadViewPreferences,
  saveViewPreferences,
  shouldUseExplorerTagSelection
} from "./viewPreferences";

import { catalogSyncMessage } from "./app/catalog/catalogSyncNotice";
import { useCatalogProjection } from "./app/catalog/useCatalogProjection";
import { useCatalogRevision } from "./app/catalog/useCatalogRevision";
import { usePendingLibrarySync } from "./app/catalog/usePendingLibrarySync";
import {
  chartAlbumSearchQuery,
  loadChartEntryTrack
} from "./charts";
import {
  libraryIntakeAdapter,
  type LibraryIntakeApplyRequest,
  type LibraryIntakePreview,
} from "./ingest";
import type { PlaybackSnapshot } from "./playback";
import {
  type YearSelection
} from "./years";

const AddFolderDialog = lazy(async () => {
  const module = await import("./components/AddFolderDialog");
  return { default: module.AddFolderDialog };
});

const Inbox = lazy(async () => {
  const module = await import("./components/inbox/Inbox");
  return { default: module.Inbox };
});

const displayViewByDestination: Record<SidebarDestination, DisplayViewKey> = {
  Universe: "universe",
  Inbox: "inbox",
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
  Playlists: "playlists",
  History: "history",
};

const trackSearchHelp = "Fields: artist (Display Artist), aartist (Album Artist display), album, genre, year (Year), ryear (Release Year), publisher, country (Album Artist origin), title, cr (album rating completeness), and love (album loved-track count). Every field accepts : or =. Numeric fields accept inclusive ranges such as year:1985..1987, cr:50..80, and love:1..3; either bound may be omitted. The shorthands cr:80 (0–80%), love:1 (one or more), and love:0 (none) keep their meanings. Use commas or uppercase AND between groups; uppercase OR inherits the preceding field; NOT or a leading - excludes. Quote a complete value for an exact match. genre:scores includes film, TV, animation, anime, and game scores.";
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

function App() {
  const [albumMoveRequest, setAlbumMoveRequest] = useState<AlbumMoveRequest | null>(null);
  const [initialViewPreferences] = useState(loadViewPreferences);
  const explorerWorkspace = useExplorerWorkspace(initialViewPreferences);
  const {
    explorerView,
    setExplorerView,
    explorerFilters,
    setExplorerFilters,
    explorerTracks,
    setExplorerTracks,
    explorerAlbums,
    setExplorerAlbums,
    explorerArtists,
    explorerCursor,
    explorerCount,
    explorerLoadState,
    setExplorerLoadState,
    explorerError,
    isLoadingMore,
    setIsLoadingMore,
    explorerReloadToken,
    setExplorerReloadToken,
    explorerSelection,
    setExplorerSelection,
    exploreRequestRef,
    loadedExplorerRequestKeyRef,
    explorerRestorationPendingRef,
    preserveExplorerOnReloadRef,
    pendingExplorerAlbumIdRef
  } = explorerWorkspace;
  const {
    settingsOpen,
    setSettingsOpen,
    settingsInitialTab,
    shortcutStatus,
    shortcutSaving,
    shortcutError,
    audioStatus,
    audioSaving,
    audioError,
    laptopModeStatus,
    laptopModeBusy,
    laptopModeError,
    toggleLaptopMode,
    saveGlobalShortcuts,
    saveAudioSettings,
    openSettings
  } = useAppSettings(() => setExplorerReloadToken(value => value + 1));
  const workspaceRestoration = useWorkspaceRestoration();
  const { restoringScrollRef, scrollReadyRef, mainScrollRef, scrollPositionByDestinationRef } = workspaceRestoration;
  const navigationState = useNavigationState();
  const { artistPageName, setArtistPageName, visitedArtists, setVisitedArtists, navigationRevision, setNavigationRevision, navigationHistory } = navigationState;
  const [snapshot, setSnapshot] = useState<LibrarySnapshot | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const selectedTrack = useAppSlice("selectedTrack");
  const { setSelectedTrack, setActiveDestination: setActiveNavState, setCatalogRevision: setChartReloadToken } = useAppStore();

  const activeNav = useAppSlice("activeDestination");

  const [layoutPreferences, setLayoutPreferences] = useState(loadLayoutPreferences);
  const [displayPreferences, setDisplayPreferences] = useState(loadDisplayPreferences);
  const [reloadToken, setReloadToken] = useState(0);
  const [selectedArtistId, setSelectedArtistId] = useState<string | null>(null);
  const [syncMessage, setSyncMessage] = useState<string | null>(null);
  const [queueOpen, setQueueOpen] = useState(false);
  const [addFolderOpen, setAddFolderOpen] = useState(false);
  const [backgroundIntakeNotice, setBackgroundIntakeNotice] = useState<{
    status: "running" | "completed" | "failed";
    message: string;
  } | null>(null);
  const backgroundIntakeRunningRef = useRef(false);

  const [chartSelection, setChartSelection] = useState<ChartSelectionContext | null>(null);
  const [chartPlaybackBusy, setChartPlaybackBusy] = useState(false);

  const searchRef = useRef<HTMLInputElement>(null);

  const loadedPageRequestsRef = useRef(new Map<string, string>());

  const appMountedRef = useRef(true);
  const activeNavRef = useRef(activeNav);
  const updater = useAuroraUpdater();
  const playback = usePlayback();
  const appendPlayback = playback.append;
  const rebindPlaybackCatalog = playback.rebindCatalog;

  const {
    inspectorArtistName, setInspectorArtistName, inspectorArtistNameRef,
    artistDetail, setArtistDetail, artistIntelligence, setArtistIntelligence,
    artistWorldState, setArtistWorldState, artistWorldError, setArtistWorldError,
    artistRequestRef, openArtistInspector, openArtistInspectorRef,
  } = useArtistNavigation({
    onShowArtistInspector: () => setInspectorView("artist"),
    onClearCurationError: () => setCurationError(null),
  });
  const {
    inspectorView,
    setInspectorView,
    inspectorViewRef,
    tagSelectionKind,
    setTagSelectionKind,
    selectedAlbumId,
    setSelectedAlbumId,
    selectedAlbumIdRef,
    selectedTrackRef,
    albumTracks,
    setAlbumTracks,
    albumTracksTruncated,
    setAlbumTracksTruncated,
    albumDetailState,
    setAlbumDetailState,
    setAlbumFileRefreshRequest,
    albumRequestRef,
    selectTrack,
    selectAlbum,
    playExplorerAlbum
  } = useInspectorSelection({
    initialViewPreferences, selectedTrack, setSelectedTrack, artistRequestRef, playback,
    setExplorerAlbums, endGenreQueue: () => endGenreQueue(),
    setSyncMessage: (value) => setSyncMessage(value),
  });

  const {
    genreAtlasGenres,
    setGenreAtlasGenres,
    selectedGenre,
    setSelectedGenre,
    genreDetail,
    setGenreDetail,
    genreSearch,
    setGenreSearch,
    genreIndexState,
    genreDetailState,
    genreIndexError,
    genreDetailError,
    setGenreIndexReloadToken,
    setGenreDetailReloadToken,
    genreQueueBusy,
    genreQueueMessage,
    genreRadioSession,
    endGenreQueue,
    startGenreQueue,
  } = useGenresDomain({
    activeNav,
    loadedPageRequestsRef,
    playback,
    appendPlayback,
    libraryReady: snapshot !== null,
  });

  const {
    savedPlaylists,
    selectedPlaylistId,
    setSelectedPlaylistId,
    playlistsLoading,
    playlistsError,
    setPlaylistsReloadToken,
    startPlaylistQueue,
  } = usePlaylistsDomain({
    playback,
    appendPlayback,
    libraryReady: snapshot !== null,
    endGenreQueue,
    selectTrack,
  });

  const {
    publisherOverview,
    publisherDetail,
    publisherLoadState,
    publisherDetailState,
    publisherError,
    publisherDetailError,
    publisherSearch,
    setPublisherSearch,
    setPublisherReloadToken,
    publisherQueueBusy,
    publisherQueueMessage,
    selectedPublisherAlbum,
    publisherAlbumTracks,
    setPublisherAlbumTracks,
    publisherAlbumBusy,
    selectPublisher,
    openPublisherAlbum,
    playPublisher,
    playPublisherAlbum,
  } = usePublishersDomain({
    setInspectorView,
    setTagSelectionKind,
    activeNav,
    loadedPageRequestsRef,
    inspectorViewRef,
    playback,
    libraryReady: snapshot !== null,
    endGenreQueue,
    selectTrack,
  });

  const {
    yearOverview,
    yearDetail,
    yearLoadState,
    yearDetailState,
    yearError,
    yearDetailError,
    setYearReloadToken,
    yearQueueBusy,
    yearQueueMessage,
    selectedYearAlbum,
    yearAlbumTracks,
    setYearAlbumTracks,
    yearAlbumBusy,
    selectYear,
    openYearAlbum,
    playYear,
    playYearAlbum,
  } = useYearsDomain({
    setInspectorView,
    setTagSelectionKind,
    activeNav,
    inspectorViewRef,
    playback,
    libraryReady: snapshot !== null,
    endGenreQueue,
    selectTrack,
  });

  const {
    ratingsOverview,
    ratingsPage,
    ratingsLoadState,
    ratingsPageState,
    ratingsError,
    ratingsPageError,
    setRatingsReloadToken,
    ratingsRefreshing,
    setRatingsRefreshing,
    setRatingsCompletion,
    ratingsRemainingTracks,
    setRatingsRemainingTracks,
    selectedRatingAlbum,
    ratingAlbumTracks,
    setRatingAlbumTracks,
    ratingsQueueBusy,
    ratingsQueueMessage,
    openRatingAlbum,
    playRatingCollection,
    playRatingAlbumUnrated,
    playTonightAlbum,
  } = useRatingsDomain({
    setInspectorView,
    setTagSelectionKind,
    activeNav,
    loadedPageRequestsRef,
    inspectorViewRef,
    playback,
    libraryReady: snapshot !== null,
    endGenreQueue,
    selectTrack,
  });

  const {
    historyPage,
    historyLoadState,
    historyError,
    historySearch,
    setHistorySearch,
    historyOutcome,
    setHistoryOutcome,
    historyDeviceId,
    setHistoryDeviceId,
    historyDateRange,
    setHistoryDateRange,
    historyLoadingMore,
    setHistoryLoadingMore,
    historySavingThreshold,
    historyThresholdMessage,
    setHistoryReloadToken,
    universeHistoryPage,
    loadMoreHistory,
    savePlayedThreshold,
    playHistoryTrack,
  } = useHistoryDomain({
    activeNav,
    loadedPageRequestsRef,
    playback,
    libraryReady: snapshot !== null,
    endGenreQueue,
    selectTrack,
  });

  const {
    curationError,
    setCurationError,
    curationActionBusy,
    curationMessage,
    reviewItems,
    reviewCursor,
    reviewFilter,
    setReviewFilter,
    reviewSearch,
    setReviewSearch,
    reviewLoadState,
    reviewError,
    reviewLoadingMore,
    setReviewLoadingMore,
    setReviewReloadToken,
    applyArtistDecision,
    applyReleaseDecision,
    loadMoreReviewItems,
    undoCuration,
    exportCuration,
  } = useObservatoryDomain({
    setInspectorView,
    inspectorArtistName,
    setInspectorArtistName,
    setArtistDetail,
    setArtistIntelligence,
    setArtistWorldState,
    activeNav,
    artistRequestRef,
    loadedPageRequestsRef,
    libraryReady: snapshot !== null,
  });

  useLayoutEffect(() => { activeNavRef.current = activeNav; }, [activeNav]);

  const { trackHistory, catalogChartRanks } = useInspectorMetadata({
    selectedTrack, setSelectedTrack, selectedAlbumId, albumTracks, explorerView,
    explorerTracks, explorerAlbums, selectedYearAlbum, snapshot, yearAlbumTracks,
    ratingAlbumTracks, publisherAlbumTracks, genreDetail,
  });

  const libraryReady = snapshot !== null;
  const {
    latestTagProjectionTokenRef,
    latestTrackProjectionTokensRef,
    acceptTrackProjectionKeys,
  } = useCatalogProjection();

  const onCatalogRefresh = useCallback((nextSnapshot: LibrarySnapshot, rebound: PlaybackSnapshot) => {
    setLoadError(null);
    setSnapshot(nextSnapshot);
    const selectedKey = selectedTrackRef.current?.trackKey;
    if (selectedKey) {
      const reboundSelection = rebound.queue.find((track) => track.trackKey === selectedKey)
        ?? nextSnapshot.tracks.find((track) => track.trackKey === selectedKey);
      if (reboundSelection) setSelectedTrack(reboundSelection);
    }

    const currentScroll = mainScrollRef.current?.scrollTop;
    if (typeof currentScroll === "number" && currentScroll > 0) {
      scrollPositionByDestinationRef.current[activeNavRef.current] = currentScroll;
    }
    restoringScrollRef.current = true;
    preserveExplorerOnReloadRef.current = true;
    if (selectedAlbumIdRef.current) pendingExplorerAlbumIdRef.current = selectedAlbumIdRef.current;
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
  }, [inspectorArtistNameRef, inspectorViewRef, mainScrollRef, openArtistInspectorRef, pendingExplorerAlbumIdRef, preserveExplorerOnReloadRef, restoringScrollRef, scrollPositionByDestinationRef, selectedAlbumIdRef, selectedTrackRef, setChartReloadToken, setExplorerReloadToken, setGenreDetailReloadToken, setGenreIndexReloadToken, setHistoryReloadToken, setPublisherReloadToken, setReviewReloadToken, setSelectedTrack, setYearReloadToken]);

  const { catalogRevisionRef, refreshCatalogIfChanged } = useCatalogRevision({
    libraryReady,
    latestTagProjectionTokenRef,
    rebindPlaybackCatalog,
    onCatalogRefresh,
  });

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
  }, [setAlbumTracks, setExplorerTracks, setGenreDetail, setPublisherAlbumTracks, setRatingAlbumTracks, setSelectedTrack, setYearAlbumTracks]);

  const onChartsChanged = useCallback(() => setChartReloadToken(value => value + 1), [setChartReloadToken]);
  const { catalogSyncNotice, handleCatalogSync } = usePendingLibrarySync({
    libraryReady,
    reloadToken,
    refreshCatalogIfChanged,
    acceptTrackProjectionKeys,
    onReconciliationChanges: applyReconciliationChanges,
    onChartsChanged,
    setSyncMessage,
  });

  const {
    inlineSavingKeys, inlineTagRevisions, applyTrackChanges, refreshTagEditorCatalogViews, saveInlineTagChange,
  } = useTagMutations({
    snapshot, setSnapshot, selectedTrack, setSelectedTrack, activeNav, setSyncMessage,
    explorer: explorerWorkspace,
    inspector: { selectedAlbumId, albumTracks, setAlbumTracks, setAlbumTracksTruncated, setAlbumDetailState, albumRequestRef },
    workspace: workspaceRestoration,
    genres: { setGenreDetail, setGenreAtlasGenres, setGenreIndexReloadToken, setGenreDetailReloadToken },
    years: { yearAlbumTracks, setYearAlbumTracks },
    ratings: { ratingAlbumTracks, setRatingAlbumTracks },
    publishers: { publisherAlbumTracks, setPublisherAlbumTracks },
    projection: { acceptTrackProjectionKeys, latestTrackProjectionTokensRef },
    handleCatalogSync,
    playback,
  });

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

  const pageKey = artistPageName ? `artist:${artistPageName}` : activeNav;
  const scrollExplorerView = explorerViewForDestination(activeNav);
  useLayoutEffect(() => {
    scrollReadyRef.current = loadError !== null || (snapshot !== null && (artistPageName !== null || scrollExplorerView === null
      || explorerLoadState === "error"
      || (explorerLoadState === "ready" && !explorerRestorationPendingRef.current && albumDetailState !== "loading"
        && loadedExplorerRequestKeyRef.current === explorerRequestKey(explorerView, explorerFilters, explorerReloadToken))));

    scrollReadyRef.current = scrollReadyRef.current && (artistPageName !== null || ({
      History: historyLoadState !== "loading",
      Observatory: reviewLoadState !== "loading",
      Genres: genreIndexState !== "loading" && genreDetailState !== "loading",
      Publishers: publisherLoadState !== "loading" && publisherDetailState !== "loading",
      Years: yearLoadState !== "loading" && yearDetailState !== "loading",
      Ratings: ratingsLoadState !== "loading" && ratingsPageState !== "loading",
    } as Partial<Record<SidebarDestination, boolean>>)[activeNav] !== false);

  });

  useWorkspaceCheckpoint({
    workspace: workspaceRestoration, explorer: explorerWorkspace, selectedTrack,
    pageKey, artistPageName, navigationRevision
  });

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
  }, [catalogRevisionRef, reloadToken, setSelectedTrack]);

  useEffect(() => {
    function focusSearch(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLocaleLowerCase() === "k") {
        event.preventDefault();
        const input = searchRef.current;
        if (!input) return;
        preparePopulatedInputForFocus(input);
        input.focus();
      }
    }
    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  }, []);

  const startBackgroundIntake = useCallback((
    request: LibraryIntakeApplyRequest,
    preview: LibraryIntakePreview,
  ) => {
    if (backgroundIntakeRunningRef.current) return;
    backgroundIntakeRunningRef.current = true;
    setBackgroundIntakeNotice({
      status: "running",
      message: `Adding ${formatCount(preview.albumCount)} ${preview.albumCount === 1 ? "album" : "albums"} in the background · Aurora remains usable`,
    });
    void libraryIntakeAdapter.apply(request)
      .then(async (result) => {
        const refreshed = await refreshCatalogIfChanged();
        if (!appMountedRef.current) return;
        if (result.cleanupWarnings.length > 0 || result.status === "completedWithWarnings") {
          setBackgroundIntakeNotice({
            status: "failed",
            message: `Albums cataloged with attention needed · ${result.cleanupWarnings[0] ?? "Open Add Music for details."}`,
          });
          return;
        }
        setBackgroundIntakeNotice({
          status: "completed",
          message: `${formatCount(result.albumCount)} ${result.albumCount === 1 ? "album" : "albums"} added · catalog updated and available covers checked${refreshed ? "" : " · Aurora refresh is catching up"}`,
        });
      })
      .catch((error: unknown) => {
        if (!appMountedRef.current) return;
        setBackgroundIntakeNotice({
          status: "failed",
          message: `Add Music needs attention · ${error instanceof Error ? error.message : String(error)}`,
        });
      })
      .finally(() => {
        backgroundIntakeRunningRef.current = false;
      });
  }, [refreshCatalogIfChanged]);

  useEffect(() => {
    if (backgroundIntakeNotice?.status !== "completed") return;
    const completedNotice = backgroundIntakeNotice;
    const timeout = window.setTimeout(() => {
      setBackgroundIntakeNotice((current) => current === completedNotice ? null : current);
    }, 10_000);
    return () => window.clearTimeout(timeout);
  }, [backgroundIntakeNotice]);

  const { loadMoreExplorerResults } = useExplorerResults({
    explorer: explorerWorkspace, workspace: workspaceRestoration, activeNav, artistPageName, libraryReady,
    initialSelectedAlbumId: initialViewPreferences.selectedAlbumId,
    selection: {
      selectedAlbumIdRef, selectedTrackRef, albumRequestRef, setSelectedAlbumId, setSelectedTrack,
      setAlbumTracks, setAlbumTracksTruncated, setAlbumDetailState, setAlbumFileRefreshRequest
    },
  });

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

  function explorePublisher(publisher: string) {
    transitionContent(() => {

      setActiveNav("Albums");
      expandLibraryNavigation();
      setExplorerView("albums");
      setExplorerFilters({
        ...defaultExplorerFilters,
        query: `publisher:"${publisher.replace(/"/g, '\\"')}"`,
        sort: "releaseYearDesc",
      });
    }, "page");
  }

  function exploreYear(selection: YearSelection) {
    transitionContent(() => {

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
    }, "page");
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
    transitionContent(() => {

      if (!chartSelection) return;
      expandLibraryNavigation();
      if (chartSelection.kind === "albums") {
        setActiveNav("Albums");
        setExplorerView("albums");
        setExplorerFilters({ ...defaultExplorerFilters, query: chartAlbumSearchQuery(chartSelection.entry), sort: "yearDesc" });
        return;
      }
      setActiveNav("Songs");
      setExplorerView("tracks");
      setExplorerFilters({ ...defaultExplorerFilters, query: chartSelection.entry.title, sort: "artistAsc" });
    }, "page");
  }

  function goToRatingAlbum(album: RatingAlbum) {
    transitionContent(() => {

      pendingExplorerAlbumIdRef.current = album.id;
      setSelectedAlbumId(album.id);
      setActiveNav("Albums");
      expandLibraryNavigation();
      setExplorerView("albums");
      setExplorerFilters(defaultExplorerFilters);
    }, "page");
  }

  function openTrackAlbum(track: Track) {
    if (!track.albumId) return;
    transitionContent(() => {
      pendingExplorerAlbumIdRef.current = track.albumId;
      setSelectedAlbumId(track.albumId);
      setInspectorView("album");
      setTagSelectionKind("album");
      setActiveNav("Albums");
      expandLibraryNavigation();
      setExplorerView("albums");
      setExplorerFilters({
        ...defaultExplorerFilters,
        query: chartAlbumSearchQuery({ title: track.album, matchedAlbumTitle: null }),
        sort: "yearDesc",
      });
    }, "page");
  }

  function exploreRatingCollection(mode: RatingMode, rating: number | null) {
    transitionContent(() => {

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
    }, "page");
  }

  function changeExplorerView(view: ExplorerView) {
    if (view === explorerView) return;
    setExplorerSelection(null);
    setExplorerView(view);
    setExplorerFilters((current) => explorerSorts[view].includes(current.sort)
      ? current
      : { ...current, sort: defaultExplorerSort[view] });
    if (view !== "albums") setSelectedAlbumId(null);
  }

  const { rememberCurrentView, goBack, setActiveNav, navigate } = useWorkspaceNavigation({
    navigation: navigationState, workspace: workspaceRestoration, explorer: explorerWorkspace, activeNav, setActiveNavState,
    setHistoryLoadingMore, setReviewLoadingMore, changeExplorerView, expandLibraryNavigation,
    selection: {
      selectedAlbumId, setSelectedAlbumId, albumTracks, setAlbumTracks, albumTracksTruncated, setAlbumTracksTruncated,
      albumDetailState, setAlbumDetailState, selectedArtistId, setSelectedArtistId, selectedTrack, setSelectedTrack,
      inspectorView, setInspectorView, tagSelectionKind, setTagSelectionKind, inspectorArtistName, setInspectorArtistName,
      artistDetail, setArtistDetail, artistIntelligence, setArtistIntelligence, artistWorldState, setArtistWorldState,
      artistWorldError, setArtistWorldError, albumRequestRef, artistRequestRef
    },
  });

  function expandLibraryNavigation() {
    setLayoutPreferences((current) => current.libraryExpanded
      ? current
      : { ...current, libraryExpanded: true });
  }

  function focusArtist(artist: Artist, destination: "tracks" | "albums" = "tracks") {
    transitionContent(() => {
      setSelectedArtistId(artist.id);
      setActiveNav(destination === "albums" ? "Albums" : "Artists");
      expandLibraryNavigation();
      setExplorerView(destination);
      setExplorerFilters((current) => ({ ...current, artist: artist.name, sort: defaultExplorerSort[destination] }));
      if (destination === "albums") setSelectedAlbumId(null);
      openArtistInspector(artist.name);
    }, "artist-detail");
  }

  function exploreArtistInLibrary(artistName: string) {
    transitionContent(() => {

      setActiveNav("Artists");
      expandLibraryNavigation();
      setExplorerView("tracks");
      setExplorerFilters((current) => ({ ...current, artist: artistName, sort: "newest" }));
    }, "page");
  }

  function openArtistAlbums(artistName: string) {
    const artist = artistName.trim();
    if (!artist || artist === artistPageName) return;
    transitionContent(() => {
      rememberCurrentView(true);
      setVisitedArtists((current) => current.includes(artist) ? current : [...current, artist].slice(-50));
      setNavigationRevision((value) => value + 1);
      setArtistPageName(artist);
    }, "page");
  }

  function exploreGenreInLibrary(genre: string) {
    transitionContent(() => {

      setActiveNav("Songs");
      expandLibraryNavigation();
      setExplorerView("tracks");
      setExplorerFilters({ ...defaultExplorerFilters, genre, sort: "newest" });
    }, "page");
  }

  async function handleAlbumRemoved(albumId: string, warnings: string[], destination: string) {
    // Invalidate pending requests before removing the row, so stale detail/page responses cannot restore it.
    if (selectedAlbumIdRef.current === albumId) albumRequestRef.current += 1;
    exploreRequestRef.current += 1;
    setIsLoadingMore(false);
    setExplorerLoadState("ready");
    setExplorerAlbums((current) => current.filter((album) => album.id !== albumId));
    setExplorerTracks((current) => current.filter((track) => track.albumId !== albumId));
    setExplorerSelection((current) => current === null ? null : current.kind === "albums"
      ? { ...current, albums: current.albums.filter((album) => album.id !== albumId) }
      : { ...current, tracks: current.tracks.filter((track) => track.albumId !== albumId) });
    setSelectedAlbumId((current) => current === albumId ? null : current);
    setAlbumTracks((current) => current.filter((track) => track.albumId !== albumId));
    if (selectedTrackRef.current?.albumId === albumId) setSelectedTrack(null);
    const message = warnings.length > 0
      ? `Album removed from Music Library. ${warnings.join(" ")}`
      : `Album moved to ${destination} and removed from Music Library.`;
    try {
      await refreshCatalogIfChanged();
      setSyncMessage(message);
    } catch (error) {
      setSyncMessage(`${message} Catalog refresh needs a retry: ${error instanceof Error ? error.message : String(error)}`);
      throw error;
    }
  }

  async function deleteExplorerAlbumTracks(tracks: readonly Track[]) {
    const albumId = tracks[0]?.albumId;
    if (!albumId || tracks.some((track) => track.albumId !== albumId)) {
      throw new Error("Every selected track must belong to the open album.");
    }
    try {
      const result = await deleteAlbumTracks(albumId, tracks);
      const deletedKeys = new Set(result.deletedTrackKeys);
      const deletedTracks = tracks.filter((track) => deletedKeys.has(track.trackKey));
      if (deletedTracks.length === 0) {
        throw new Error(result.failures[0]?.message ?? "Aurora could not delete the selected tracks.");
      }
      // A catalog or album request that began before deletion must not restore its stale row.
      albumRequestRef.current += 1;
      exploreRequestRef.current += 1;
      setIsLoadingMore(false);
      setExplorerLoadState("ready");
      const isDeleted = (candidate: Track) => deletedKeys.has(candidate.trackKey);
      const selectedAlbum = explorerAlbums.find((album) => album.id === albumId);
      const removesAlbum = Boolean(selectedAlbum && selectedAlbum.totalTracks <= deletedTracks.length);
      const deletedRated = deletedTracks.filter((track) => track.rating !== null).length;
      const deletedLoved = deletedTracks.filter((track) => track.loved).length;
      const deletedDuration = deletedTracks.reduce((total, track) => total + (track.durationSeconds ?? 0), 0);

      setAlbumTracks((current) => current.filter((candidate) => !isDeleted(candidate)));
      setExplorerTracks((current) => current.filter((candidate) => !isDeleted(candidate)));
      setYearAlbumTracks((current) => current.filter((candidate) => !isDeleted(candidate)));
      setRatingAlbumTracks((current) => current.filter((candidate) => !isDeleted(candidate)));
      setPublisherAlbumTracks((current) => current.filter((candidate) => !isDeleted(candidate)));
      setExplorerAlbums((current) => removesAlbum
        ? current.filter((album) => album.id !== albumId)
        : current.map((album) => album.id === albumId ? applyAlbumTrackMetricsProjection({
          ...album,
          totalTracks: Math.max(0, album.totalTracks - deletedTracks.length),
          ratedTracks: Math.max(0, album.ratedTracks - deletedRated),
          lovedTracks: Math.max(0, album.lovedTracks - deletedLoved),
          durationSeconds: album.durationSeconds === null
            ? null
            : Math.max(0, album.durationSeconds - deletedDuration),
        }, albumTracks.filter((candidate) => !isDeleted(candidate))) : album));
      setSnapshot((current) => current ? {
        ...current,
        tracks: current.tracks.filter((candidate) => !isDeleted(candidate)),
        summary: {
          ...current.summary,
          songs: Math.max(0, current.summary.songs - deletedTracks.length),
          albums: Math.max(0, current.summary.albums - (removesAlbum ? 1 : 0)),
          rated: Math.max(0, current.summary.rated - deletedRated),
          loved: Math.max(0, current.summary.loved - deletedLoved),
        },
      } : current);
      setGenreDetail((current) => current ? {
        ...current,
        highlights: current.highlights.filter((candidate) => !isDeleted(candidate)),
      } : current);
      if (selectedTrackRef.current && deletedKeys.has(selectedTrackRef.current.trackKey)) setSelectedTrack(null);
      if (removesAlbum) setSelectedAlbumId(null);

      const deletedLabel = `${formatCount(deletedTracks.length)} ${deletedTracks.length === 1 ? "track" : "tracks"}`;
      const failureLabel = result.failures.length > 0
        ? ` · ${formatCount(result.failures.length)} could not be deleted`
        : "";
      setSyncMessage(`Deleted ${deletedLabel} from disk${failureLabel} · notifying Music Library`);
      await handleCatalogSync(result.catalogSync, true);
    } catch (error) {
      setSyncMessage(`Could not delete the selected tracks: ${error instanceof Error ? error.message : String(error)}`);
      throw error;
    }
  }

  function submitSearch(event: FormEvent) {
    event.preventDefault();
  }

  const explorerAlbumInspectorContext = artistPageName === null && explorerViewForDestination(activeNav) !== null && explorerView === "albums"
    ? resolveExplorerAlbumInspectorContext(explorerAlbums, selectedAlbumId, albumTracks, selectedTrack)
    : null;
  const inspectorTrack = explorerAlbumInspectorContext
    ? explorerAlbumInspectorContext.track
    : selectedTrack;
  const inspectorAlbumYear = inspectorTrack?.originalYear ?? inspectorTrack?.releaseYear;
  const inspectorAlbumLabel = inspectorTrack
    ? `${inspectorTrack.album}${inspectorAlbumYear ? ` (${inspectorAlbumYear})` : ""}`
    : "";
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
  const explorerTagTarget = !shouldUseExplorerTagSelection(activeNav)
    ? undefined
    : explorerSelection?.kind === "tracks"
      ? explorerSelection.tracks.length === 0
        ? null
        : explorerSelection.tracks.length === 1
          ? {
            kind: "track" as const,
            trackId: explorerSelection.tracks[0].id,
            trackKey: explorerSelection.tracks[0].trackKey,
            label: explorerSelection.tracks[0].title,
          }
          : {
            kind: "tracks" as const,
            tracks: explorerSelection.tracks.map((track) => ({ trackId: track.id, trackKey: track.trackKey })),
            label: `${formatCount(explorerSelection.tracks.length)} tracks selected`,
          }
      : explorerSelection?.kind === "albums"
        ? explorerSelection.albums.length === 0
          ? null
          : explorerSelection.albums.length === 1
            ? { kind: "album" as const, albumId: explorerSelection.albums[0].id, label: explorerSelection.albums[0].title }
            : {
              kind: "albums" as const,
              albumIds: explorerSelection.albums.map((album) => album.id),
              label: `${formatCount(explorerSelection.albums.length)} albums selected`,
            }
        : undefined;
  const tagEditorTarget = explorerTagTarget !== undefined
    ? explorerTagTarget
    : tagSelectionKind === "album"
      ? albumTagTarget
      : inspectorTrack
        ? { kind: "track" as const, trackId: inspectorTrack.id, trackKey: inspectorTrack.trackKey, label: inspectorTrack.title }
        : null;
  const tagEditorKey = !tagEditorTarget
    ? "none"
    : tagEditorTarget.kind === "album"
      ? `album:${tagEditorTarget.albumId}:${albumTracks.length}`
      : tagEditorTarget.kind === "track"
        ? `track:${tagEditorTarget.trackKey}:${inlineTagRevisions[tagEditorTarget.trackKey] ?? 0}`
        : tagEditorTarget.kind === "albums"
          ? `albums:${tagEditorTarget.albumIds.join("|")}`
          : `tracks:${tagEditorTarget.tracks.map((track) => `${track.trackKey}:${inlineTagRevisions[track.trackKey] ?? 0}`).join("|")}`;

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
    && artistPageName === null
    && !["Inbox", "Observatory", "Charts", "Playlists", "History", "Genres", "Publishers", "Years", "Ratings"].includes(activeNav);
  const topbarSearchValue = activeNav === "Playlists" || activeNav === "Inbox"
    ? ""
    : activeNav === "Observatory"
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
  const topbarSearchPlaceholder = activeNav === "Playlists"
    ? "Playlist search is coming soon…"
    : activeNav === "Inbox"
      ? "Inbox search is coming after folder monitoring…"
      : activeNav === "Observatory"
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
  const topbarSearchLabel = activeNav === "Playlists"
    ? "Playlist search is not available yet"
    : activeNav === "Inbox"
      ? "Inbox search is not available yet"
      : activeNav === "Observatory"
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
    if (artistPageName) {
      rememberCurrentView(true);
      setNavigationRevision((revision) => revision + 1);
    }
    setArtistPageName(null);
    if (activeNav === "Inbox") return;
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
      data-right-sidebar={artistPageName || activeNav === "Playlists" ? "collapsed" : layoutPreferences.rightSidebar}
      data-inbox={activeNav === "Inbox" ? "true" : undefined}
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
          playlists={savedPlaylists}
          selectedPlaylistId={selectedPlaylistId}
          playlistsLoading={playlistsLoading}
          playlistsError={playlistsError}
          onLibraryExpandedChange={(libraryExpanded) => setLayoutPreferences((current) => ({
            ...current,
            libraryExpanded,
          }))}
          onPlaylistsExpandedChange={(playlistsExpanded) => setLayoutPreferences((current) => ({
            ...current,
            playlistsExpanded,
          }))}
          onNavigate={navigate}
          onSelectPlaylist={(id) => { setSelectedPlaylistId(id); navigate("Playlists"); }}
        />

        <div className="profile">
          <CircleUserRound aria-hidden="true" />
          <span><strong>Jørn</strong><small>Aurora 0.28.21</small></span>
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
          <form className={`search${activeNav === "Years" || activeNav === "Inbox" || activeNav === "Playlists" ? " is-disabled" : ""}`} role="search" onSubmit={submitSearch}>
            <Search aria-hidden="true" />
            <input
              ref={searchRef}
              value={topbarSearchValue}
              onPointerDownCapture={(event) => {
                if (!preparePopulatedInputForFocus(event.currentTarget)) return;
                event.preventDefault();
                event.currentTarget.focus();
              }}
              onChange={(event) => updateTopbarSearch(event.target.value)}
              placeholder={topbarSearchPlaceholder}
              aria-label={topbarSearchLabel}
              title={explorerView === "tracks" && !["Inbox", "Observatory", "History", "Genres", "Publishers", "Years", "Playlists"].includes(activeNav) ? trackSearchHelp : undefined}
              disabled={activeNav === "Years" || activeNav === "Inbox" || activeNav === "Playlists"}
            />
            {topbarSearchValue
              ? <button type="button" aria-label="Clear search" onClick={() => updateTopbarSearch("")}><X aria-hidden="true" /></button>
              : activeNav !== "Years" && activeNav !== "Inbox" && activeNav !== "Playlists" ? <kbd>Ctrl K</kbd> : null}
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
          <div className="topbar__status">
            {backgroundIntakeNotice && (
              <span
                className="tag-sync-message intake-background-message"
                data-intake-status={backgroundIntakeNotice.status}
                role={backgroundIntakeNotice.status === "failed" ? "alert" : "status"}
                title={backgroundIntakeNotice.message}
              >{backgroundIntakeNotice.message}</span>
            )}
            {!backgroundIntakeNotice && syncMessage && <span className="tag-sync-message" role="status">{syncMessage}</span>}
            {!backgroundIntakeNotice && catalogNoticeMessage && (
              <span
                className="tag-sync-message"
                data-sync-status={catalogSyncNotice?.status}
                role="status"
                title={catalogNoticeMessage}
              >{catalogNoticeMessage}</span>
            )}
          </div>
        </div>
        <div className="topbar__actions">
          <button type="button" className="add-music-action" disabled={backgroundIntakeNotice?.status === "running"} onClick={() => setAddFolderOpen(true)}><FolderPlus aria-hidden="true" /><span>{backgroundIntakeNotice?.status === "running" ? "Adding music" : "Add music"}</span></button>
          <LaptopModeButton
            status={laptopModeStatus}
            busy={laptopModeBusy}
            error={laptopModeError}
            onToggle={() => void toggleLaptopMode()}
          />
          <button type="button" aria-label="Audio settings" title="Audio settings" onClick={() => openSettings("audio")}><AudioLines aria-hidden="true" /></button>
          <button type="button" aria-label="Labs" disabled><FlaskConical aria-hidden="true" /></button>
          {!updater.state.version && <button type="button" aria-label="Check for updates" title="Check for updates" onClick={() => void updater.checkForUpdate(true)}><Download aria-hidden="true" /></button>}
          {updater.state.version && <button type="button" className="update-badge" onClick={updater.showPrompt}><Download aria-hidden="true" /> Update {updater.state.version}</button>}
          {activeNav !== "Playlists" && <button
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
          </button>}
          <button type="button" aria-label="Settings" title="Settings" onClick={() => openSettings("display")}><Settings aria-hidden="true" /></button>
        </div>
      </header>

      <main className="main-content">
        <div className="page-navigation">
          <button type="button" onClick={goBack} disabled={navigationHistory.length === 0}
            className="page-back" title="Return to the previous page and view">
            <ArrowLeft aria-hidden="true" />
            {navigationHistory.length ? `Back to ${navigationHistory[navigationHistory.length - 1]!.artist ?? navigationHistory[navigationHistory.length - 1]!.destination}` : "Back"}
          </button>
        </div>
        <div
          className="main-scroll"
          ref={mainScrollRef}
          onScroll={workspaceRestoration.rememberScroll}
          data-page-key={pageKey}
          data-text-size={activeDisplayPreferences.textSize}
          data-cover-size={activeDisplayPreferences.coverSize}
        >
          {snapshot ? (
            <ContentTransition type="page">
              {visitedArtists.map((artist) => (
                <RememberedPage key={artist} active={artistPageName === artist}>
                  <ArtistRoute artist={artist} onOpenArtist={openArtistAlbums}
                    onOpenAlbum={(album) => {
                      transitionContent(() => {
                        pendingExplorerAlbumIdRef.current = album.id;
                        setSelectedAlbumId(album.id);
                        setInspectorView("album");
                        setTagSelectionKind("album");
                        setArtistPageName(null);
                        setActiveNav("Albums");
                        setExplorerView("albums");
                        setExplorerFilters({ ...defaultExplorerFilters, artist: album.artist, sort: "yearDesc" });
                        setExplorerReloadToken((value) => value + 1);
                      }, "page");
                    }}
                    onPlay={playChartQueue} onSettings={() => openSettings("metadata")} />
                </RememberedPage>
              ))}
              <RememberedPage active={!artistPageName && activeNav === "Inbox"}>
                <Suspense fallback={<section className="inbox-load" aria-live="polite">Opening Inbox…</section>}>
                  <Inbox
                    onOpenMetadataSettings={() => openSettings("metadata")}
                    onCatalogChanged={refreshCatalogIfChanged}
                  />
                </Suspense>
              </RememberedPage>
              <RememberedPage active={!artistPageName && activeNav === "Observatory"}>
                <ObservatoryRoute
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
              </RememberedPage>
              <RememberedPage active={!artistPageName && activeNav === "Charts"}>
                <ChartsRoute
                  onOpenArtistAlbums={openArtistAlbums}
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
              </RememberedPage>
              <RememberedPage active={!artistPageName && activeNav === "Playlists"}>
                <PlaylistsRoute
                  active={!artistPageName && activeNav === "Playlists"}
                  playlists={savedPlaylists}
                  selectedId={selectedPlaylistId}
                  listLoading={playlistsLoading}
                  listError={playlistsError}
                  onSelect={setSelectedPlaylistId}
                  onRefresh={() => setPlaylistsReloadToken((value) => value + 1)}
                  onPlay={startPlaylistQueue}
                />
              </RememberedPage>
              <RememberedPage active={!artistPageName && activeNav === "History"}>
                <HistoryRoute
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
                  onOpenArtistAlbums={openArtistAlbums}
                  onLoadMore={() => void loadMoreHistory()}
                  onRefresh={() => setHistoryReloadToken((value) => value + 1)}
                />
              </RememberedPage>
              <RememberedPage active={!artistPageName && activeNav === "Genres"}>
                <GenresRoute
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
                  onSelectGenre={(genre) => transitionContent(() => setSelectedGenre(genre), "collection")}
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
              </RememberedPage>
              <RememberedPage active={!artistPageName && activeNav === "Publishers"}>
                <PublishersRoute
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
              </RememberedPage>
              <RememberedPage active={!artistPageName && activeNav === "Years"}>
                <YearsRoute
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
              </RememberedPage>
              <RememberedPage active={!artistPageName && activeNav === "Ratings"}>
                <RatingsRoute
                  tonight={{ onPlay: playTonightAlbum, onOpen: goToRatingAlbum, onSettings: () => openSettings("connections"), playbackBusy: ratingsQueueBusy }}
                  overview={ratingsOverview}
                  page={ratingsPage}
                  selectedAlbum={selectedRatingAlbum}
                  albumTracks={ratingAlbumTracks}
                  loadState={ratingsLoadState}
                  pageState={ratingsPageState}
                  errorMessage={ratingsError}
                  pageError={ratingsPageError}
                  queueBusy={ratingsQueueBusy}
                  refreshing={ratingsRefreshing}
                  queueMessage={ratingsQueueMessage}
                  busyTrackKeys={inlineSavingKeys}
                  remainingTracks={ratingsRemainingTracks}
                  onCompletionChange={(value) => transitionContent(() => setRatingsCompletion(value), "collection")}
                  onRemainingTracksChange={(value) => transitionContent(() => setRatingsRemainingTracks(value), "collection")}
                  onSelectAlbum={openRatingAlbum}
                  onGoToAlbum={goToRatingAlbum}
                  onSelectTrack={selectTrack}
                  onPlayTrack={(track) => playTrack(track, ratingAlbumTracks)}
                  onRatingChange={(track, rating) => void saveInlineTagChange(track, { ...tagValuesForTrack(track), rating })}
                  onLoveChange={(track, loveState) => void saveInlineTagChange(track, { ...tagValuesForTrack(track), loveState })}
                  onPlayCollection={(mode, rating) => void playRatingCollection(mode, rating)}
                  onExploreCollection={exploreRatingCollection}
                  onPlayUnrated={(album) => void playRatingAlbumUnrated(album)}
                  onRefresh={() => {
                    if (ratingsRefreshing) return;
                    setRatingsRefreshing(true);
                    setRatingsReloadToken((value) => value + 1);
                  }}
                  onRetry={() => setRatingsReloadToken((value) => value + 1)}
                  onRetryPage={() => setRatingsReloadToken((value) => value + 1)}
                />
              </RememberedPage>
              <ReactActivity mode={showExplorerCount ? "visible" : "hidden"}>
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
                  {universeHistoryPage && <UniverseListeningMemory page={universeHistoryPage} onOpenHistory={() => navigate("History")} />}
                </> : null}

                {(["Universe", "Songs", "Albums", "Artists", "Tags"] as const).map((destination) => (
                  <RememberedPage key={destination} active={showExplorerCount && activeNav === destination}>
                    <LibraryRoute
                      view={explorerView}
                      filters={explorerFilters}
                      tracks={explorerTracks}
                      albums={explorerAlbums}
                      artists={explorerArtists}
                      currentTrackKey={playback.state.currentTrack?.trackKey ?? null}
                      playbackActive={playback.state.status === "playing"}
                      selectedAlbumId={selectedAlbumId}
                      selectedArtistId={selectedArtistId}
                      albumTracks={albumTracks}
                      albumTracksTruncated={albumTracksTruncated}
                      trackChartRanks={catalogChartRanks.tracks}
                      albumChartRanks={catalogChartRanks.albums}
                      loadState={explorerLoadState}
                      errorMessage={explorerError}
                      albumDetailState={albumDetailState}
                      pageInfo={{ loaded: explorerLoaded, hasMore: explorerCursor !== null, isLoadingMore }}
                      busyTrackKeys={inlineSavingKeys}
                      onViewChange={changeExplorerView}
                      onFiltersChange={(filters) => {
                        setExplorerSelection(null);
                        setExplorerFilters(filters);
                      }}
                      onSelectTrack={selectTrack}
                      onActivateTrack={(track) => playTrack(track, albumTracks.some((candidate) => candidate.id === track.id) ? albumTracks : explorerTracks)}
                      onSelectAlbum={selectAlbum}
                      onSelectArtist={(artist) => { if (artist) openArtistAlbums(artist.name); else setSelectedArtistId(null); }}
                      onOpenArtistAlbums={openArtistAlbums}
                      onLoadMore={() => void loadMoreExplorerResults()}
                      onRetry={() => {
                        if (selectedAlbumId && albumDetailState === "error") {
                          const album = explorerAlbums.find((candidate) => candidate.id === selectedAlbumId);
                          if (album) selectAlbum(album);
                        } else {
                          setExplorerReloadToken((value) => value + 1);
                        }
                      }}
                      onClearFilters={() => {
                        setExplorerSelection(null);
                        setExplorerFilters({ ...defaultExplorerFilters, sort: defaultExplorerSort[explorerView] });
                      }}
                      onRatingChange={(track, rating) => void saveInlineTagChange(track, { ...tagValuesForTrack(track), rating })}
                      onLoveChange={(track, loveState) => void saveInlineTagChange(track, { ...tagValuesForTrack(track), loveState })}
                      onDeleteTracks={deleteExplorerAlbumTracks}
                      onRequestMoveToInbox={(album) => setAlbumMoveRequest((current) => current ?? { album, mode: "inbox" })}
                      onRequestRemoveAlbum={(album) => setAlbumMoveRequest((current) => current ?? { album, mode: "remove" })}
                      albumMoveBusy={albumMoveRequest !== null}
                      onSelectionChange={(selection) => {
                        setExplorerSelection(selection);
                        setTagSelectionKind(selection.kind === "albums" ? "album" : "track");
                      }}
                    />
                  </RememberedPage>
                ))}
              </ReactActivity>
            </ContentTransition>
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

      {!artistPageName && activeNav !== "Inbox" && activeNav !== "Playlists" && layoutPreferences.rightSidebar === "expanded" && <aside
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
              key={tagEditorKey}
              target={tagEditorTarget}
              onTracksChange={applyTrackChanges}
              onCatalogSync={refreshTagEditorCatalogViews}
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
              onOpenAlbum={openTrackAlbum}
              onOpenArtistAlbums={openArtistAlbums}
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
              onOpenArtistAlbums={openArtistAlbums}
              chartRanks={catalogChartRanks.albums[explorerAlbumInspectorContext.album.id]}
              ratingDigits={2}
            />
            <RemoveAlbumButton disabled={albumMoveRequest !== null} onRequest={() => setAlbumMoveRequest((current) => current ?? { album: explorerAlbumInspectorContext.album, mode: "remove" })} />
          </div>
        ) : inspectorView === "album" && activeNav === "Publishers" && selectedPublisherAlbum ? (
          <div className="inspector-scroll">
            <ContentTransition type="collection"><PublisherAlbumInspector album={selectedPublisherAlbum} busy={publisherAlbumBusy} onPlay={(album) => void playPublisherAlbum(album)} onOpenArtistAlbums={openArtistAlbums} /></ContentTransition>
          </div>
        ) : inspectorView === "album" && activeNav === "Ratings" && selectedRatingAlbum ? (
          <div className="inspector-scroll">
            <ContentTransition type="collection"><RatingAlbumInspector album={selectedRatingAlbum} busy={ratingsQueueBusy} onPlay={(album) => void playRatingAlbumUnrated(album)} onOpenArtistAlbums={openArtistAlbums} /></ContentTransition>
          </div>
        ) : inspectorView === "album" && activeNav === "Years" && selectedYearAlbum ? (
          <div className="inspector-scroll">
            <ContentTransition type="collection"><YearAlbumInspector album={selectedYearAlbum} busy={yearAlbumBusy} onPlay={(album) => void playYearAlbum(album)} onOpenArtistAlbums={openArtistAlbums} chartRanks={catalogChartRanks.albums[selectedYearAlbum.id]} /></ContentTransition>
          </div>
        ) : inspectorView === "artist" && inspectorArtistName ? (
          <div className="inspector-scroll">
            <ContentTransition type="artist-detail">
              <ArtistInspector
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
            </ContentTransition>
          </div>
        ) : inspectorTrack ? (
          <div className="inspector-scroll">
            <Artwork track={inspectorTrack} size="large" />
            <div className="track-hero-copy">
              <div><h2>{inspectorTrack.title}</h2><p><ArtistSmartLink artist={displayTrackArtist(inspectorTrack)} onOpen={openArtistAlbums} /></p>{inspectorTrack.albumId ? <button type="button" className="track-album-link" onClick={() => openTrackAlbum(inspectorTrack)} title={`Open ${inspectorTrack.album} in Albums`}>{inspectorTrack.album}</button> : <span>{inspectorTrack.album}</span>}</div>
              <button type="button" className="inspector-play" onClick={() => playTrack(inspectorTrack)}><Play aria-hidden="true" /> Play</button>
            </div>
            <dl className="metadata-list">
              {catalogChartRanks.tracks[inspectorTrack.id]?.length ? <div><dt>Charts</dt><dd><CatalogChartRanks kind="track" ranks={catalogChartRanks.tracks[inspectorTrack.id]} /></dd></div> : null}
              <div className="track-album-metadata"><dt>Album</dt><dd>{inspectorTrack.albumId ? <button type="button" className="track-album-link" onClick={() => openTrackAlbum(inspectorTrack)} title={`Open ${inspectorTrack.album} in Albums`}>{inspectorAlbumLabel}</button> : <span>{inspectorAlbumLabel}</span>}</dd></div>
              <div className="publisher-metadata"><dt>Publisher</dt><dd>{inspectorTrack.publisher ?? "Unknown"}</dd></div>
              <div><dt>Genre</dt><dd>{inspectorTrack.genre ?? "Unknown"}</dd></div>
              <div><dt>Last.fm popularity</dt><dd>{inspectorTrack.playCount === null ? "—" : formatCount(inspectorTrack.playCount)}</dd></div>
              <div><dt>Duration</dt><dd>{formatDuration(inspectorTrack.durationSeconds)}</dd></div>
              <div><dt>Your registered plays</dt><dd>{trackHistory?.trackKey === inspectorTrack.trackKey ? formatCount(trackHistory.value.plays) : "—"}</dd></div>
              <div><dt>Your listening time</dt><dd>{trackHistory?.trackKey === inspectorTrack.trackKey ? formatDuration(Math.round(trackHistory.value.listenedSeconds)) : "—"}</dd></div>
              <div><dt>Last listened</dt><dd>{trackHistory?.trackKey === inspectorTrack.trackKey ? historyDateLabel(trackHistory.value.lastListenedAtMs) : "—"}</dd></div>
            </dl>
            <TrackChartInfo artist={displayTrackArtist(inspectorTrack)} title={inspectorTrack.title} />
            <div className="readonly-note"><BadgeCheck aria-hidden="true" /><span><strong>Verified file writes</strong>Use the Tags tab to edit this MP3 or the selected album without leaving Aurora.</span></div>
          </div>
        ) : <EmptyInspector />}
      </aside>}

      {albumMoveRequest ? <AlbumMoveOperation request={albumMoveRequest} onDismiss={() => setAlbumMoveRequest(null)} onRemoved={handleAlbumRemoved} /> : null}

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
        onOpenArtistAlbums={openArtistAlbums}
        onOpenAudioSettings={() => openSettings("audio")}
        onToggleQueue={() => setQueueOpen((open) => !open)}
      />

      {addFolderOpen && (
        <Suspense fallback={<div className="modal-backdrop"><div className="settings-loading" role="status">Opening album intake…</div></div>}>
          <AddFolderDialog
            onClose={() => setAddFolderOpen(false)}
            onCatalogChanged={refreshCatalogIfChanged}
            onApplyInBackground={startBackgroundIntake}
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

export default function AuroraApp() {
  return <AppStoreProvider><App /></AppStoreProvider>;
}
