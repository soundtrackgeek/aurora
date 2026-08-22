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
  Gauge,
  Heart,
  LibraryBig,
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
  Sparkles,
  Star,
  Tags,
  Telescope,
  UsersRound,
  X,
} from "lucide-react";
import { type FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import "./App.css";
import { Artwork } from "./components/Artwork";
import { LaptopModeButton } from "./components/LaptopModeButton";
import {
  DeepExplorer,
  type ExplorerAlbum,
  type ExplorerFilters,
  type ExplorerLoadState,
  type ExplorerSort,
  type ExplorerView,
} from "./components/explorer/DeepExplorer";
import { ArtistWorld, type ArtistWorldState } from "./components/musicbrainz/ArtistWorld";
import { Observatory, type ObservatoryLoadState } from "./components/curation/Observatory";
import {
  ListeningHistory,
  type HistoryDateRange,
  type HistoryLoadState,
} from "./components/history/ListeningHistory";
import { PlayerBar } from "./components/PlayerBar";
import { QueuePanel } from "./components/QueuePanel";
import { SettingsDialog } from "./components/SettingsDialog";
import { TagEditor } from "./components/TagEditor";
import {
  exploreAlbums,
  exploreArtists,
  exploreTracks,
  formatCount,
  formatDuration,
  loadAlbumDetail,
  loadArtistDetail,
  loadLibrarySnapshot,
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
  reconcilePendingTags,
  tagValuesForTrack,
  trackWithReconciledTags,
  trackWithTagValues,
  updateTrackTags,
  type TagReconciliationChange,
  type TagValues,
} from "./tags";
import { useAuroraUpdater } from "./updater";
import {
  listenForGlobalShortcutResults,
  loadGlobalShortcutSettings,
  updateGlobalShortcutSettings,
  type GlobalShortcutSettingsRequest,
  type GlobalShortcutStatus,
  type GlobalShortcutResult,
} from "./shortcuts";

const navigation = [
  { label: "Universe", icon: Sparkles },
  { label: "Observatory", icon: Telescope },
  { label: "Library", icon: LibraryBig },
  { label: "Albums", icon: Album },
  { label: "Artists", icon: UsersRound },
  { label: "Genres", icon: Disc3 },
  { label: "Songs", icon: Music2 },
  { label: "Ratings", icon: Star },
  { label: "Tags", icon: Tags },
  { label: "History", icon: Clock3 },
];

const previewPlaylists = [
  { label: "5 Star Collection", count: "rating view", icon: Star },
  { label: "Night Drive", count: "smart playlist", icon: Music2 },
  { label: "Unplayed", count: "listening queue", icon: Disc3 },
];

const defaultExplorerFilters: ExplorerFilters = {
  query: "",
  rating: "all",
  love: "all",
  yearFrom: null,
  yearTo: null,
  genre: null,
  artist: null,
  sort: "newest",
};

const explorerSorts: Record<ExplorerView, readonly ExplorerSort[]> = {
  tracks: ["newest", "titleAsc", "artistAsc", "albumAsc", "releaseYearDesc", "ratingDesc"],
  albums: ["releaseYearDesc", "titleAsc", "artistAsc", "ratingDesc"],
  artists: ["artistAsc", "trackCountDesc"],
};

const defaultSort: Record<ExplorerView, ExplorerSort> = {
  tracks: "newest",
  albums: "releaseYearDesc",
  artists: "artistAsc",
};

type ExplorerResult = {
  tracks: Track[];
  albums: AlbumSummary[];
  artists: Artist[];
  nextCursor: ExplorerCursor | null;
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
      artist: filters.artist ?? undefined,
      sort: explorerSorts.tracks.includes(filters.sort)
        ? filters.sort as "newest" | "titleAsc" | "artistAsc" | "albumAsc" | "releaseYearDesc" | "ratingDesc"
        : "newest",
    });
    return { tracks: page.items, albums: [], artists: [], nextCursor: page.nextCursor };
  }
  if (view === "albums") {
    const page = await exploreAlbums({
      ...shared,
      yearFrom: filters.yearFrom ?? undefined,
      yearTo: filters.yearTo ?? undefined,
      artist: filters.artist ?? undefined,
      sort: explorerSorts.albums.includes(filters.sort)
        ? filters.sort as "titleAsc" | "artistAsc" | "releaseYearDesc" | "ratingDesc"
        : "releaseYearDesc",
    });
    return { tracks: [], albums: page.items, artists: [], nextCursor: page.nextCursor };
  }
  const page = await exploreArtists({
    ...shared,
    sort: filters.sort === "trackCountDesc" ? "trackCountDesc" : "nameAsc",
  });
  return { tracks: [], albums: [], artists: page.items, nextCursor: page.nextCursor };
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
  const [snapshot, setSnapshot] = useState<LibrarySnapshot | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selectedTrack, setSelectedTrack] = useState<Track | null>(null);
  const [trackHistory, setTrackHistory] = useState<{ trackKey: string; value: TrackHistoryInsight } | null>(null);
  const [inspectorView, setInspectorView] = useState<"track" | "artist">("track");
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
  const [activeNav, setActiveNav] = useState("Universe");
  const [layoutPreferences, setLayoutPreferences] = useState(loadLayoutPreferences);
  const [reloadToken, setReloadToken] = useState(0);
  const [explorerView, setExplorerView] = useState<ExplorerView>("tracks");
  const [explorerFilters, setExplorerFilters] = useState<ExplorerFilters>(defaultExplorerFilters);
  const [explorerTracks, setExplorerTracks] = useState<Track[]>([]);
  const [explorerAlbums, setExplorerAlbums] = useState<AlbumSummary[]>([]);
  const [explorerArtists, setExplorerArtists] = useState<Artist[]>([]);
  const [explorerCursor, setExplorerCursor] = useState<ExplorerCursor | null>(null);
  const [explorerLoadState, setExplorerLoadState] = useState<ExplorerLoadState>("loading");
  const [explorerError, setExplorerError] = useState<string | null>(null);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [explorerReloadToken, setExplorerReloadToken] = useState(0);
  const [selectedAlbumId, setSelectedAlbumId] = useState<string | null>(null);
  const [albumTracks, setAlbumTracks] = useState<Track[]>([]);
  const [albumTracksTruncated, setAlbumTracksTruncated] = useState(false);
  const [albumDetailState, setAlbumDetailState] = useState<ExplorerLoadState>("ready");
  const [selectedArtistId, setSelectedArtistId] = useState<string | null>(null);
  const [syncMessage, setSyncMessage] = useState<string | null>(null);
  const [queueOpen, setQueueOpen] = useState(false);
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
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [shortcutStatus, setShortcutStatus] = useState<GlobalShortcutStatus | null>(null);
  const [shortcutSaving, setShortcutSaving] = useState(false);
  const [shortcutError, setShortcutError] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const exploreRequestRef = useRef(0);
  const albumRequestRef = useRef(0);
  const artistRequestRef = useRef(0);
  const reviewRequestRef = useRef(0);
  const historyRequestRef = useRef(0);
  const reconciliationRunningRef = useRef(false);
  const inlineSaveRef = useRef<Set<string>>(new Set());
  const shortcutResultHandlerRef = useRef<(result: GlobalShortcutResult) => void>(() => undefined);
  const updater = useAuroraUpdater();
  const playback = usePlayback();

  useEffect(() => {
    saveLayoutPreferences(layoutPreferences);
  }, [layoutPreferences]);

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
    void loadGlobalShortcutSettings()
      .then((status) => {
        if (!cancelled) setShortcutStatus(status);
      })
      .catch((error: unknown) => {
        if (!cancelled) setShortcutError(error instanceof Error ? error.message : String(error));
      });
    return () => { cancelled = true; };
  }, [settingsOpen]);

  useEffect(() => {
    let cancelled = false;
    void loadLibrarySnapshot()
      .then((nextSnapshot) => {
        if (cancelled) return;
        setSnapshot(nextSnapshot);
        setSelectedTrack(nextSnapshot.tracks[0] ?? null);
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

  useEffect(() => {
    if (!libraryReady || activeNav === "Observatory" || activeNav === "History") return;
    const requestId = ++exploreRequestRef.current;
    let cancelled = false;
    albumRequestRef.current += 1;
    const clearDetailTimer = window.setTimeout(() => {
      if (cancelled) return;
      setIsLoadingMore(false);
      setSelectedAlbumId(null);
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
          setExplorerLoadState("ready");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== exploreRequestRef.current) return;
          setExplorerError(error instanceof Error ? error.message : String(error));
          setExplorerLoadState("error");
        });
    }, explorerFilters.query.trim() ? 160 : 0);
    return () => {
      cancelled = true;
      window.clearTimeout(clearDetailTimer);
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, explorerView, explorerFilters, explorerReloadToken]);

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
    setSelectedTrack((current) => current ? reconcile(current) : current);
    setSnapshot((current) => current ? { ...current, tracks: current.tracks.map(reconcile) } : current);
  }, []);

  const refreshExternalTagChanges = useCallback(async () => {
    if (reconciliationRunningRef.current) return;
    reconciliationRunningRef.current = true;
    let totalChanges = 0;
    let totalIssues = 0;
    let hasMore = false;
    let firstIssue: string | null = null;
    try {
      for (let batch = 0; batch < 10; batch += 1) {
        const report = await reconcilePendingTags();
        applyReconciliationChanges(report.changes);
        totalChanges += report.externalChanges;
        totalIssues += report.issues.length;
        firstIssue ??= report.issues[0]?.message ?? null;
        hasMore = report.hasMore;
        if (!report.hasMore || report.processed === 0) break;
      }
      if (totalChanges > 0) {
        setSyncMessage(`Refreshed ${formatCount(totalChanges)} external tag ${totalChanges === 1 ? "change" : "changes"}`);
      } else if (totalIssues > 0 || hasMore) {
        setSyncMessage(totalIssues === 1 && firstIssue
          ? firstIssue
          : `${formatCount(totalIssues)} tag ${totalIssues === 1 ? "item needs" : "items need"} attention`);
      } else {
        setSyncMessage(null);
      }
    } catch (error) {
      console.warn("Aurora could not reconcile pending tags", error);
      setSyncMessage("Tag refresh will retry when Aurora regains focus");
    } finally {
      reconciliationRunningRef.current = false;
    }
  }, [applyReconciliationChanges]);

  useEffect(() => {
    if (!libraryReady) return;
    const initialRefresh = window.setTimeout(() => void refreshExternalTagChanges(), 0);
    const refreshOnFocus = () => void refreshExternalTagChanges();
    window.addEventListener("focus", refreshOnFocus);
    return () => {
      window.clearTimeout(initialRefresh);
      window.removeEventListener("focus", refreshOnFocus);
    };
  }, [libraryReady, reloadToken, refreshExternalTagChanges]);

  function playTrack(
    track: Track,
    queue = albumTracks.some((candidate) => candidate.id === track.id)
      ? albumTracks
      : explorerTracks.length > 0
        ? explorerTracks
        : snapshot?.tracks ?? [],
  ) {
    selectTrack(track);
    void playback.play(queue, track.id);
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
    setInspectorView("track");
  }

  function applyTrackChange(updated: Track, previous?: Track, updateSelected = true) {
    const baseline = previous ?? (selectedTrack?.id === updated.id ? selectedTrack : undefined);
    if (updateSelected) {
      setSelectedTrack((current) => current?.id === updated.id ? updated : current);
    }
    setExplorerTracks((current) => current.map((track) => track.id === updated.id ? updated : track));
    setAlbumTracks((current) => current.map((track) => track.id === updated.id ? updated : track));
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
        tracks: current.tracks.map((track) => track.id === updated.id ? updated : track),
      };
    });
  }

  shortcutResultHandlerRef.current = (result) => {
    setSyncMessage(result.success ? result.message : `Shortcut failed: ${result.message}`);
    if (result.track) applyTrackChange(result.track, result.previousTrack ?? undefined);
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

  async function saveInlineTagChange(track: Track, desired: TagValues) {
    if (inlineSaveRef.current.has(track.trackKey)) return;
    const expected = tagValuesForTrack(track);
    if (JSON.stringify(expected) === JSON.stringify(desired)) return;

    inlineSaveRef.current.add(track.trackKey);
    setInlineSavingKeys((current) => new Set(current).add(track.trackKey));
    setSyncMessage(null);

    const optimistic = trackWithTagValues(track, desired);
    applyTrackChange(optimistic, track, false);
    try {
      const snapshot = await updateTrackTags(track, expected, desired);
      applyTrackChange(snapshot.track, optimistic);
      setInlineTagRevisions((current) => ({
        ...current,
        [track.trackKey]: (current[track.trackKey] ?? 0) + 1,
      }));
    } catch (error) {
      applyTrackChange(track, optimistic, false);
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
      sort: explorerSorts[view].includes(current.sort) ? current.sort : defaultSort[view],
    }));
    if (view !== "albums") setSelectedAlbumId(null);
  }

  function navigate(label: string) {
    setActiveNav(label);
    if (label === "Observatory" || label === "History") return;
    if (label === "Albums") changeExplorerView("albums");
    else if (label === "Artists") changeExplorerView("artists");
    else changeExplorerView("tracks");
  }

  function focusArtist(artist: Artist) {
    setSelectedArtistId(artist.id);
    setActiveNav("Artists");
    setExplorerView("tracks");
    setExplorerFilters((current) => ({ ...current, artist: artist.name, sort: "newest" }));
    openArtistInspector(artist.name);
  }

  function exploreArtistInLibrary(artistName: string) {
    setActiveNav("Artists");
    setExplorerView("tracks");
    setExplorerFilters((current) => ({ ...current, artist: artistName, sort: "newest" }));
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
    setSelectedAlbumId(album?.id ?? null);
    setAlbumTracks([]);
    setAlbumTracksTruncated(false);
    if (!album) {
      setAlbumDetailState("ready");
      return;
    }
    setAlbumDetailState("loading");
    void loadAlbumDetail(album.id)
      .then((detail) => {
        if (requestId !== albumRequestRef.current) return;
        setAlbumTracks(detail.tracks);
        setAlbumTracksTruncated(detail.tracksTruncated);
        setAlbumDetailState("ready");
      })
      .catch((error: unknown) => {
        if (requestId !== albumRequestRef.current) return;
        console.warn("Aurora could not open album details", error);
        setAlbumDetailState("error");
      });
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
    selectTrack(track);
    void playback.play([track], track.id);
  }

  function submitSearch(event: FormEvent) {
    event.preventDefault();
  }

  const genres = useMemo(() => Array.from(new Set([
    ...(snapshot?.tracks.map((track) => track.genre).filter((genre): genre is string => Boolean(genre)) ?? []),
    ...explorerTracks.map((track) => track.genre).filter((genre): genre is string => Boolean(genre)),
    ...explorerAlbums.map((album) => album.genre).filter((genre): genre is string => Boolean(genre)),
  ])).sort((left, right) => left.localeCompare(right)).slice(0, 200), [snapshot?.tracks, explorerTracks, explorerAlbums]);
  const artistOptions = useMemo(() => Array.from(new Set([
    ...(snapshot?.artists.map((artist) => artist.name) ?? []),
    ...explorerArtists.map((artist) => artist.name),
    ...explorerTracks.map((track) => track.artist),
  ])).sort((left, right) => left.localeCompare(right)).slice(0, 200), [snapshot?.artists, explorerArtists, explorerTracks]);
  const explorerLoaded = explorerView === "tracks"
    ? explorerTracks.length
    : explorerView === "albums"
      ? explorerAlbums.length
      : explorerArtists.length;

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

  return (
    <div
      className="app-shell"
      data-left-sidebar={layoutPreferences.leftSidebar}
      data-right-sidebar={layoutPreferences.rightSidebar}
    >
      {layoutPreferences.leftSidebar !== "collapsed" && <aside className="sidebar">
        <div className="brand">
          <div className="brand__mark"><AudioLines aria-hidden="true" /></div>
          <div><strong>Aurora</strong><span>your music, your universe</span></div>
        </div>

        <p className="sidebar__label">Navigation</p>
        <nav className="primary-nav" aria-label="Primary">
          {navigation.map(({ label, icon: Icon }) => (
            <button
              type="button"
              key={label}
              className={activeNav === label ? "is-active" : undefined}
              onClick={() => navigate(label)}
              aria-label={layoutPreferences.leftSidebar === "icons" ? label : undefined}
              title={layoutPreferences.leftSidebar === "icons" ? label : undefined}
            >
              <Icon aria-hidden="true" />
              <span>{label}</span>
              {activeNav === label && <ChevronRight className="nav-chevron" aria-hidden="true" />}
            </button>
          ))}
        </nav>

        <div className="sidebar__section-heading"><p className="sidebar__label">Playlists</p><button type="button" aria-label="Add playlist" disabled>+</button></div>
        <div className="playlists">
          {previewPlaylists.map(({ label, count, icon: Icon }) => (
            <button type="button" key={label} disabled>
              <Icon aria-hidden="true" /><span><strong>{label}</strong><small>{count}</small></span>
            </button>
          ))}
        </div>

        <div className="profile">
          <CircleUserRound aria-hidden="true" />
          <span><strong>Jørn</strong><small>Aurora 0.9.0</small></span>
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
          <form className="search" role="search" onSubmit={submitSearch}>
            <Search aria-hidden="true" />
            <input
              ref={searchRef}
              value={activeNav === "Observatory" ? reviewSearch : activeNav === "History" ? historySearch : explorerFilters.query}
              onChange={(event) => activeNav === "Observatory"
                ? setReviewSearch(event.target.value)
                : activeNav === "History"
                  ? setHistorySearch(event.target.value)
                  : setExplorerFilters((current) => ({ ...current, query: event.target.value }))}
              placeholder={activeNav === "Observatory" ? "Search artists to review…" : activeNav === "History" ? "Search listening history…" : "Search your universe…"}
              aria-label={activeNav === "Observatory" ? "Search MusicBrainz review artists" : activeNav === "History" ? "Search listening history" : "Search your music universe"}
            />
            {(activeNav === "Observatory" ? reviewSearch : activeNav === "History" ? historySearch : explorerFilters.query)
              ? <button type="button" aria-label="Clear search" onClick={() => activeNav === "Observatory" ? setReviewSearch("") : activeNav === "History" ? setHistorySearch("") : setExplorerFilters((current) => ({ ...current, query: "" }))}><X aria-hidden="true" /></button>
              : <kbd>Ctrl K</kbd>}
          </form>
        </div>
        <div className="topbar__actions">
          {syncMessage && <span className="tag-sync-message" role="status">{syncMessage}</span>}
          <LaptopModeButton
            status={laptopModeStatus}
            busy={laptopModeBusy}
            error={laptopModeError}
            onToggle={() => void toggleLaptopMode()}
          />
          <button type="button" aria-label="Audio tools" disabled><AudioLines aria-hidden="true" /></button>
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
          <button type="button" aria-label="Settings" title="Settings" onClick={() => {
            setShortcutError(null);
            setSettingsOpen(true);
          }}><Settings aria-hidden="true" /></button>
        </div>
      </header>

      <main className="main-content">
        <div className="main-scroll">
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
                genres={genres}
                artistOptions={artistOptions}
                selectedTrackId={selectedTrack?.id ?? null}
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
                onSelectArtist={(artist) => { if (artist) focusArtist(artist); else setSelectedArtistId(null); }}
                onLoadMore={() => void loadMoreExplorerResults()}
                onRetry={() => {
                  if (selectedAlbumId && albumDetailState === "error") {
                    const album = explorerAlbums.find((candidate) => candidate.id === selectedAlbumId);
                    if (album) selectAlbum(album);
                  } else {
                    setExplorerReloadToken((value) => value + 1);
                  }
                }}
                onClearFilters={() => setExplorerFilters({ ...defaultExplorerFilters, sort: defaultSort[explorerView] })}
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

      {layoutPreferences.rightSidebar === "expanded" && <aside className="inspector">
        <div className="inspector-tabs" role="tablist" aria-label="Track details">
          <button type="button" role="tab" aria-selected={inspectorView === "track"} disabled={!selectedTrack} onClick={() => setInspectorView("track")}>Track</button>
          <button type="button" role="tab" aria-selected="false" disabled>Album</button>
          <button
            type="button"
            role="tab"
            aria-selected={inspectorView === "artist"}
            disabled={!selectedTrack && !inspectorArtistName}
            onClick={() => {
              const artistName = selectedTrack?.artist ?? inspectorArtistName;
              if (artistName) openArtistInspector(artistName);
            }}
          >Artist</button>
          <button type="button" role="tab" aria-selected="false" disabled>Lyrics</button>
        </div>
        {inspectorView === "artist" && inspectorArtistName ? (
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
        ) : selectedTrack ? (
          <div className="inspector-scroll">
            <Artwork track={selectedTrack} size="large" />
            <div className="track-hero-copy">
              <div><h2>{selectedTrack.title}</h2><p>{selectedTrack.artist}</p><span>{selectedTrack.album}</span></div>
              <button type="button" className="inspector-play" onClick={() => playTrack(selectedTrack)}><Play aria-hidden="true" /> Play</button>
            </div>
            <dl className="metadata-list">
              <div><dt>Genre</dt><dd>{selectedTrack.genre ?? "Unknown"}</dd></div>
              <div><dt>Last.fm popularity</dt><dd>{selectedTrack.playCount === null ? "—" : formatCount(selectedTrack.playCount)}</dd></div>
              <div><dt>Duration</dt><dd>{formatDuration(selectedTrack.durationSeconds)}</dd></div>
              <div><dt>Your registered plays</dt><dd>{trackHistory?.trackKey === selectedTrack.trackKey ? formatCount(trackHistory.value.plays) : "—"}</dd></div>
              <div><dt>Your listening time</dt><dd>{trackHistory?.trackKey === selectedTrack.trackKey ? formatDuration(Math.round(trackHistory.value.listenedSeconds)) : "—"}</dd></div>
              <div><dt>Last listened</dt><dd>{trackHistory?.trackKey === selectedTrack.trackKey ? historyDateLabel(trackHistory.value.lastListenedAtMs) : "—"}</dd></div>
            </dl>
            <TagEditor key={`${selectedTrack.id}:${inlineTagRevisions[selectedTrack.trackKey] ?? 0}`} track={selectedTrack} onTrackChange={applyTrackChange} />
            <div className="readonly-note"><BadgeCheck aria-hidden="true" /><span><strong>Verified file writes</strong>Aurora edits only MusicBee rating, Love/Ban, and Release Time frames. The catalog remains read-only.</span></div>
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
          onClear={() => void playback.clear()}
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
        onToggleQueue={() => setQueueOpen((open) => !open)}
      />

      {updater.state.isPromptOpen && <UpdateDialog version={updater.state.version} phase={updater.state.phase} progress={updater.state.progress} message={updater.state.message} onInstall={() => void updater.install()} onDismiss={updater.dismiss} />}
      {settingsOpen && shortcutStatus && (
        <SettingsDialog
          status={shortcutStatus}
          saving={shortcutSaving}
          error={shortcutError}
          onSave={(request) => void saveGlobalShortcuts(request)}
          onClose={() => setSettingsOpen(false)}
        />
      )}
      {settingsOpen && !shortcutStatus && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => {
          if (event.target === event.currentTarget) setSettingsOpen(false);
        }}>
          <div className="settings-loading" role="status">
            <span>{shortcutError ?? "Loading settings…"}</span>
            {shortcutError && <button type="button" aria-label="Close settings" onClick={() => setSettingsOpen(false)}><X aria-hidden="true" /></button>}
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
