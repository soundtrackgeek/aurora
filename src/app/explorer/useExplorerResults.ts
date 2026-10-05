import { useCallback, useEffect, useState, type Dispatch, type RefObject, type SetStateAction } from "react";
import type { ExplorerFilters, ExplorerLoadState, ExplorerView } from "../../components/explorer/DeepExplorer";
import type { SidebarDestination } from "../../components/navigation/SidebarNavigation";
import { applyAlbumPopularity, applyAlbumTrackMetricsProjection, loadAlbumDetail, type Track } from "../../library";
import { transitionContent } from "../../contentTransition";
import {
  createExplorerRefreshQueue,
  mergeRefreshedExplorerPage,
  refreshedExplorerCursor,
  resolveExplorerRefreshPreservation,
  shouldReuseExplorerPage
} from "../../explorerRefresh";
import { explorerViewForDestination } from "../../viewPreferences";
import { loadWorkspacePages, restoreWorkspaceScroll } from "../../workspaceRestoration";
import { explorerCountKey, explorerRequestKey, loadExplorerPage, type ExplorerResult } from "./explorerQueries";
import type { ExplorerWorkspace } from "./useExplorerWorkspace";
import type { WorkspaceRestoration } from "../navigation/useWorkspaceRestoration";

type Setter<T> = Dispatch<SetStateAction<T>>;

/** The inspector owns selection; explorer requests only restore or invalidate it. */
export interface ExplorerSelectionPort {
  selectedAlbumIdRef: RefObject<string | null>;
  selectedTrackRef: RefObject<Track | null>;
  albumRequestRef: RefObject<number>;
  setSelectedAlbumId: Setter<string | null>;
  setSelectedTrack: Setter<Track | null>;
  setAlbumTracks: Setter<Track[]>;
  setAlbumTracksTruncated: Setter<boolean>;
  setAlbumDetailState: Setter<ExplorerLoadState>;
  setAlbumFileRefreshRequest: Setter<{ albumId: string; requestId: number; } | null>;
}

export interface ExplorerResultsOptions {
  explorer: ExplorerWorkspace;
  workspace: WorkspaceRestoration;
  selection: ExplorerSelectionPort;
  activeNav: SidebarDestination;
  artistPageName: string | null;
  libraryReady: boolean;
  initialSelectedAlbumId: string | null;
}

const explorerSearchDebounceMs = 2_000;

/** Loads bounded explorer windows and rejects work belonging to a superseded view. */
export function useExplorerResults({ explorer, workspace, selection, activeNav, artistPageName, libraryReady, initialSelectedAlbumId }: ExplorerResultsOptions) {
  const { explorerView,
    explorerFilters,
    setExplorerTracks,
    setExplorerAlbums,
    setExplorerArtists,
    explorerCursor,
    setExplorerCursor,
    setExplorerCount,
    explorerLoadState,
    setExplorerLoadState,
    setExplorerError,
    setIsLoadingMore,
    explorerReloadToken,
    exploreRequestRef,
    loadedExplorerRequestKeyRef,
    loadedExplorerViewKeyRef,
    explorerCursorRef,
    explorerLoadedRef,
    explorerLocalOnlyRef,
    explorerLoadingMoreRef,
    explorerRestorationPendingRef,
    preserveExplorerOnReloadRef,
    pendingExplorerAlbumIdRef } = explorer;
  const {
    selectedAlbumIdRef,
    selectedTrackRef,
    albumRequestRef,
    setSelectedAlbumId,
    setSelectedTrack,
    setAlbumTracks,
    setAlbumTracksTruncated,
    setAlbumDetailState,
    setAlbumFileRefreshRequest
  } = selection;
  const { initialWorkspace, mainScrollRef, scrollPositionByDestinationRef, restoringScrollRef, scrollReadyRef } = workspace;
  const [queueExplorerRefresh] = useState(() => createExplorerRefreshQueue<ExplorerResult>());

  const refreshExplorerFiles = useCallback((
    view: ExplorerView, filters: ExplorerFilters, targetCount: number, requestId: number,
    cancelled: () => boolean = () => false,
  ) => {
    const obsolete = () => cancelled() || requestId !== exploreRequestRef.current;
    queueExplorerRefresh({
      load: () => loadWorkspacePages<ExplorerResult>(
        (cursor) => loadExplorerPage(view, filters, cursor, false), targetCount, obsolete,
      ),
      cancelled: obsolete,
      apply: (page) => {
        // Replace the complete loaded window: merging would retain albums that
        // no longer satisfy a pending genre/completeness filter.
        setExplorerTracks(page.tracks);
        setExplorerAlbums(page.albums);
        setExplorerArtists(page.artists);
        setExplorerCursor(page.nextCursor);
        setExplorerCount({ key: explorerCountKey(view, filters), total: page.totalCount });
        explorerLocalOnlyRef.current = false;
        const selected = selectedAlbumIdRef.current;
        if (view === "albums" && selected && !page.albums.some((album) => album.id === selected)) {
          albumRequestRef.current += 1;
          setSelectedAlbumId(null);
          setAlbumTracks([]);
          setAlbumTracksTruncated(false);
          setAlbumDetailState("ready");
        }
      },
      failed: (error) => console.warn("Aurora kept local search results after file refresh failed", error),
    });
  }, [queueExplorerRefresh, albumRequestRef, exploreRequestRef, explorerLocalOnlyRef, selectedAlbumIdRef, setAlbumDetailState, setAlbumTracks, setAlbumTracksTruncated, setExplorerAlbums, setExplorerArtists, setExplorerCount, setExplorerCursor, setExplorerTracks, setSelectedAlbumId]);

  useEffect(() => {
    const explorerActive = libraryReady && !artistPageName && explorerViewForDestination(activeNav) !== null;
    const preservation = resolveExplorerRefreshPreservation(
      preserveExplorerOnReloadRef.current,
      explorerActive,
      loadedExplorerViewKeyRef.current === explorerRequestKey(explorerView, explorerFilters, 0),
    );
    preserveExplorerOnReloadRef.current = preservation.pending;
    if (!explorerActive) return;
    const preservingCurrentView = preservation.preservingCurrentView;
    const localOnly = !preservingCurrentView || explorerLocalOnlyRef.current;
    const requestKey = explorerRequestKey(explorerView, explorerFilters, explorerReloadToken);
    if (shouldReuseExplorerPage(loadedExplorerRequestKeyRef.current, requestKey, preservingCurrentView)) return;
    const restoringStoredView = explorerRestorationPendingRef.current;
    const handoffAlbumId = explorerView === "albums" ? pendingExplorerAlbumIdRef.current : null;
    const restoredAlbumId = handoffAlbumId
      ?? (preservingCurrentView && explorerView === "albums" ? selectedAlbumIdRef.current : null)
      ?? (restoringStoredView && explorerView === "albums" ? initialSelectedAlbumId : null);
    const restoredTrackKey = preservingCurrentView ? selectedTrackRef.current?.trackKey : restoringStoredView ? initialWorkspace.trackKey : null;
    const preservedLoaded = explorerLoadedRef.current;
    const preservedCursor = explorerCursorRef.current;
    const preservedScroll = preservingCurrentView
      ? (mainScrollRef.current?.scrollTop ?? scrollPositionByDestinationRef.current[activeNav] ?? 0)
      : null;
    if (preservedScroll !== null && preservedScroll > 0) {
      restoringScrollRef.current = true;
    }
    const requestId = ++exploreRequestRef.current;
    let cancelled = false;
    const selectionRequestId = ++albumRequestRef.current;
    const clearDetailTimer = window.setTimeout(() => {
      if (cancelled) return;
      setIsLoadingMore(false);
      if (!preservingCurrentView) {
        if (!restoredAlbumId) setSelectedAlbumId(null);
        setAlbumTracks([]);
        setAlbumTracksTruncated(false);
      }
    }, 0);
    const timer = window.setTimeout(() => {
      if (!preservingCurrentView) setExplorerLoadState("loading");
      setExplorerError(null);
      setExplorerCursor(null);
      explorerLocalOnlyRef.current = localOnly;
      void loadWorkspacePages<ExplorerResult>(
        (cursor) => loadExplorerPage(explorerView, explorerFilters, cursor, localOnly),
        restoringStoredView && initialWorkspace.explorerKey === explorerRequestKey(explorerView, explorerFilters, 0)
          ? initialWorkspace.loaded
          : (preservingCurrentView ? preservedLoaded : 0),
        () => cancelled,
      )
        .then((page) => {
          transitionContent(() => {
            if (cancelled || requestId !== exploreRequestRef.current) return;
            setExplorerTracks((current) => preservingCurrentView ? mergeRefreshedExplorerPage(current, page.tracks) : page.tracks);
            setExplorerAlbums((current) => preservingCurrentView ? mergeRefreshedExplorerPage(current, page.albums) : page.albums);
            setExplorerArtists((current) => preservingCurrentView ? mergeRefreshedExplorerPage(current, page.artists) : page.artists);
            setExplorerCursor(preservingCurrentView
              ? refreshedExplorerCursor(preservedLoaded, page.tracks.length + page.albums.length + page.artists.length, preservedCursor, page.nextCursor)
              : page.nextCursor);
            setExplorerCount({ key: explorerCountKey(explorerView, explorerFilters), total: page.totalCount });
            setExplorerLoadState("ready");
            loadedExplorerRequestKeyRef.current = requestKey;
            loadedExplorerViewKeyRef.current = explorerRequestKey(explorerView, explorerFilters, 0);
            const restoreScrollIfPreserved = () => {
              if (preservedScroll !== null && preservedScroll > 0) {
                const scrollContainer = mainScrollRef.current;
                if (scrollContainer) {
                  restoreWorkspaceScroll(
                    scrollContainer,
                    preservedScroll,
                    () => scrollReadyRef.current,
                    () => { restoringScrollRef.current = false; },
                  );
                  return;
                }
              }
              restoringScrollRef.current = false;
            };
            // Browsing during a background reload owns its own detail request.
            // Refresh the rows without restoring the album captured at reload start.
            if (selectionRequestId !== albumRequestRef.current) {
              restoringScrollRef.current = false;
            } else if (restoredAlbumId && (handoffAlbumId || preservingCurrentView || page.albums.some((album) => album.id === restoredAlbumId))) {
              const albumDetailRequestId = ++albumRequestRef.current;
              setSelectedAlbumId(restoredAlbumId);
              if (!preservingCurrentView) {
                setAlbumDetailState("loading");
              }
              void loadAlbumDetail(restoredAlbumId, { localOnly: true })
                .then((detail) => {
                  transitionContent(() => {
                    if (albumDetailRequestId !== albumRequestRef.current) return;
                    const projectedAlbum = applyAlbumTrackMetricsProjection(detail.album, detail.tracks);
                    setExplorerAlbums((current) => current.some((album) => album.id === detail.album.id)
                      ? current.map((album) => album.id === detail.album.id ? projectedAlbum : album)
                      : [projectedAlbum, ...current]);
                    setAlbumTracks(applyAlbumPopularity(detail.tracks, detail.popularity));
                    setAlbumTracksTruncated(detail.tracksTruncated);
                    setSelectedTrack(detail.tracks.find((track) => track.trackKey === restoredTrackKey) ?? detail.tracks[0] ?? null);
                    setAlbumDetailState("ready");
                    setAlbumFileRefreshRequest({ albumId: restoredAlbumId, requestId: albumDetailRequestId });
                    restoreScrollIfPreserved();
                  }, "album-detail", !preservingCurrentView);
                })
                .catch((error: unknown) => {
                  if (albumDetailRequestId !== albumRequestRef.current) return;
                  console.warn("Aurora could not restore album details", error);
                  setAlbumDetailState("error");
                  restoreScrollIfPreserved();
                });
            } else {
              if (restoringStoredView) {
                setSelectedAlbumId(null);
              }
              restoreScrollIfPreserved();
            }
            if (handoffAlbumId === pendingExplorerAlbumIdRef.current) pendingExplorerAlbumIdRef.current = null;
            explorerRestorationPendingRef.current = false;
            if (localOnly) refreshExplorerFiles(explorerView, explorerFilters,
              Math.max(preservingCurrentView ? preservedLoaded : 0, page.tracks.length + page.albums.length + page.artists.length),
              requestId, () => cancelled);
          }, "artist-detail", !preservingCurrentView);
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== exploreRequestRef.current) return;
          explorerRestorationPendingRef.current = false;
          restoringScrollRef.current = false;
          setExplorerError(error instanceof Error ? error.message : String(error));
          if (preservingCurrentView) {
            console.warn("Aurora kept the current Library view after its background refresh failed", error);
          } else {
            setExplorerLoadState("error");
          }
        });
    }, preservingCurrentView ? 0 : (explorerFilters.query.trim() ? explorerSearchDebounceMs : 0));
    return () => {
      cancelled = true;
      window.clearTimeout(clearDetailTimer);
      window.clearTimeout(timer);
    };
  }, [activeNav, artistPageName, libraryReady, explorerView, explorerFilters, explorerReloadToken, initialSelectedAlbumId, initialWorkspace, refreshExplorerFiles, albumRequestRef, exploreRequestRef, explorerCursorRef, explorerLoadedRef, explorerLocalOnlyRef, explorerRestorationPendingRef, loadedExplorerRequestKeyRef, loadedExplorerViewKeyRef, mainScrollRef, pendingExplorerAlbumIdRef, preserveExplorerOnReloadRef, restoringScrollRef, scrollPositionByDestinationRef, scrollReadyRef, selectedAlbumIdRef, selectedTrackRef, setAlbumDetailState, setAlbumFileRefreshRequest, setAlbumTracks, setAlbumTracksTruncated, setExplorerAlbums, setExplorerArtists, setExplorerCount, setExplorerCursor, setExplorerError, setExplorerLoadState, setExplorerTracks, setIsLoadingMore, setSelectedAlbumId, setSelectedTrack]);

  useEffect(() => {
    if (
      libraryReady
      && ["Inbox", "Observatory", "Charts", "Playlists", "History", "Genres", "Publishers", "Years", "Ratings"].includes(activeNav)
    ) explorerRestorationPendingRef.current = false;
  }, [activeNav, libraryReady, explorerRestorationPendingRef]);

  async function loadMoreExplorerResults() {
    // During debounce, the visible cursor can still belong to the previous search.
    // Pagination must not cancel its replacement request or append across searches.
    if (!explorerCursor || explorerLoadingMoreRef.current
      || explorerLoadState !== "ready"
      || !shouldReuseExplorerPage(loadedExplorerRequestKeyRef.current,
        explorerRequestKey(explorerView, explorerFilters, explorerReloadToken), false)) return;
    const requestId = ++exploreRequestRef.current;
    const localOnly = explorerLocalOnlyRef.current;
    const loadedBefore = explorerLoadedRef.current;
    explorerLoadingMoreRef.current = true;
    setIsLoadingMore(true);
    try {
      const page = await loadExplorerPage(explorerView, explorerFilters, explorerCursor, localOnly);
      if (requestId !== exploreRequestRef.current) return;
      setExplorerTracks((current) => [...current, ...page.tracks]);
      setExplorerAlbums((current) => [...current, ...page.albums]);
      setExplorerArtists((current) => [...current, ...page.artists]);
      setExplorerCursor(page.nextCursor);
      setExplorerCount({ key: explorerCountKey(explorerView, explorerFilters), total: page.totalCount });
      if (localOnly) refreshExplorerFiles(explorerView, explorerFilters,
        loadedBefore + page.tracks.length + page.albums.length + page.artists.length, requestId);
    } catch (error) {
      if (requestId === exploreRequestRef.current) {
        setExplorerError(error instanceof Error ? error.message : String(error));
        setExplorerLoadState("error");
      }
    } finally {
      explorerLoadingMoreRef.current = false;
      if (requestId === exploreRequestRef.current) setIsLoadingMore(false);
    }
  }

  return { loadMoreExplorerResults };
}
