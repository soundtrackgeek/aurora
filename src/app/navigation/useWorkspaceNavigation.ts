import { useRef, useState, type Dispatch, type RefObject, type SetStateAction } from "react";
import type { ExplorerFilters, ExplorerLoadState, ExplorerSelection, ExplorerView } from "../../components/explorer/DeepExplorer";
import type { SidebarDestination } from "../../components/navigation/SidebarNavigation";
import type { ArtistWorldState } from "../../components/musicbrainz/ArtistWorld";
import type { Track, AlbumSummary, Artist, ExplorerCursor, ArtistDetail } from "../../library";
import type { ArtistIntelligence } from "../../musicbrainz";
import { defaultExplorerFilters, defaultExplorerSort, explorerViewForDestination, type InspectorView, type TagSelectionKind } from "../../viewPreferences";
import { transitionContent } from "../../contentTransition";
import { saveWorkspaceCheckpoint } from "../../workspaceRestoration";
import type { ExplorerWorkspace } from "../explorer/useExplorerWorkspace";
import type { WorkspaceRestoration } from "./useWorkspaceRestoration";

type Setter<T> = Dispatch<SetStateAction<T>>;

export interface NavigationView {
  destination: SidebarDestination;
  artist: string | null;
  key: string;
  scroll: number;
  explorerView: ExplorerView;
  explorerFilters: ExplorerFilters;
  explorerTracks: Track[];
  explorerAlbums: AlbumSummary[];
  explorerArtists: Artist[];
  explorerCursor: ExplorerCursor | null;
  explorerCount: { key: string; total: number; } | null;
  explorerLoadState: ExplorerLoadState;
  explorerError: string | null;
  explorerSelection: ExplorerSelection | null;
  selectedAlbumId: string | null;
  albumTracks: Track[];
  albumTracksTruncated: boolean;
  albumDetailState: ExplorerLoadState;
  selectedArtistId: string | null;
  selectedTrack: Track | null;
  inspectorView: InspectorView;
  tagSelectionKind: TagSelectionKind;
  inspectorArtistName: string | null;
  artistDetail: ArtistDetail | null;
  artistIntelligence: ArtistIntelligence | null;
  artistWorldState: ArtistWorldState;
  artistWorldError: string | null;
  loadedKey: string | null;
  viewKey: string | null;
  localOnly: boolean;
  revision: number;
}

export interface NavigationSelectionPort {
  selectedAlbumId: string | null;
  setSelectedAlbumId: Setter<string | null>;
  albumTracks: Track[];
  setAlbumTracks: Setter<Track[]>;
  albumTracksTruncated: boolean;
  setAlbumTracksTruncated: Setter<boolean>;
  albumDetailState: ExplorerLoadState;
  setAlbumDetailState: Setter<ExplorerLoadState>;
  selectedArtistId: string | null;
  setSelectedArtistId: Setter<string | null>;
  selectedTrack: Track | null;
  setSelectedTrack: Setter<Track | null>;
  inspectorView: InspectorView;
  setInspectorView: Setter<InspectorView>;
  tagSelectionKind: TagSelectionKind;
  setTagSelectionKind: Setter<TagSelectionKind>;
  inspectorArtistName: string | null;
  setInspectorArtistName: Setter<string | null>;
  artistDetail: ArtistDetail | null;
  setArtistDetail: Setter<ArtistDetail | null>;
  artistIntelligence: ArtistIntelligence | null;
  setArtistIntelligence: Setter<ArtistIntelligence | null>;
  artistWorldState: ArtistWorldState;
  setArtistWorldState: Setter<ArtistWorldState>;
  artistWorldError: string | null;
  setArtistWorldError: Setter<string | null>;
  albumRequestRef: RefObject<number>;
  artistRequestRef: RefObject<number>;
}

export function useNavigationState() {
  const [artistPageName, setArtistPageName] = useState<string | null>(null);
  const [visitedArtists, setVisitedArtists] = useState<string[]>([]);
  const [navigationRevision, setNavigationRevision] = useState(0);
  const [navigationHistory, setNavigationHistory] = useState<NavigationView[]>([]);
  const savedViewsRef = useRef(new Map<string, NavigationView>());
  return { artistPageName, setArtistPageName, visitedArtists, setVisitedArtists, navigationRevision, setNavigationRevision, navigationHistory, setNavigationHistory, savedViewsRef };
}

export type NavigationState = ReturnType<typeof useNavigationState>;

interface WorkspaceNavigationOptions {
  navigation: NavigationState;
  workspace: WorkspaceRestoration;
  explorer: ExplorerWorkspace;
  selection: NavigationSelectionPort;
  activeNav: SidebarDestination;
  setActiveNavState: Setter<SidebarDestination>;
  setHistoryLoadingMore: Setter<boolean>;
  setReviewLoadingMore: Setter<boolean>;
  changeExplorerView: (view: ExplorerView) => void;
  expandLibraryNavigation: () => void;
}

/** Retains destination snapshots and invalidates outstanding work before crossing views. */
export function useWorkspaceNavigation({
  navigation,
  workspace,
  explorer,
  selection,
  activeNav,
  setActiveNavState,
  setHistoryLoadingMore,
  setReviewLoadingMore,
  changeExplorerView,
  expandLibraryNavigation
}: WorkspaceNavigationOptions) {
  const { artistPageName, setArtistPageName, setNavigationRevision, navigationHistory, setNavigationHistory, savedViewsRef } = navigation;
  const { workspaceRef, restoringScrollRef, mainScrollRef, scrollPositionByDestinationRef } = workspace;
  const { explorerView,
    setExplorerView,
    explorerFilters,
    setExplorerFilters,
    explorerTracks,
    setExplorerTracks,
    explorerAlbums,
    setExplorerAlbums,
    explorerArtists,
    setExplorerArtists,
    explorerCursor,
    setExplorerCursor,
    explorerCount,
    setExplorerCount,
    explorerLoadState,
    setExplorerLoadState,
    explorerError,
    setExplorerError,
    setIsLoadingMore,
    explorerReloadToken,
    explorerSelection,
    setExplorerSelection,
    exploreRequestRef,
    loadedExplorerRequestKeyRef,
    loadedExplorerViewKeyRef,
    explorerLocalOnlyRef,
    explorerLoadingMoreRef,
    preserveExplorerOnReloadRef } = explorer;
  const {
    selectedAlbumId,
    setSelectedAlbumId,
    albumTracks,
    setAlbumTracks,
    albumTracksTruncated,
    setAlbumTracksTruncated,
    albumDetailState,
    setAlbumDetailState,
    selectedArtistId,
    setSelectedArtistId,
    selectedTrack,
    setSelectedTrack,
    inspectorView,
    setInspectorView,
    tagSelectionKind,
    setTagSelectionKind,
    inspectorArtistName,
    setInspectorArtistName,
    artistDetail,
    setArtistDetail,
    artistIntelligence,
    setArtistIntelligence,
    artistWorldState,
    setArtistWorldState,
    artistWorldError,
    setArtistWorldError,
    albumRequestRef,
    artistRequestRef
  } = selection;
  const pageKey = artistPageName ? `artist:${artistPageName}` : activeNav;

  function captureNavigationView(): NavigationView {
    return {
      destination: activeNav, artist: artistPageName, key: pageKey,
      scroll: restoringScrollRef.current ? scrollPositionByDestinationRef.current[pageKey] ?? 0 : mainScrollRef.current?.scrollTop ?? 0,
      explorerView, explorerFilters, explorerTracks, explorerAlbums, explorerArtists,
      explorerCursor, explorerCount, explorerLoadState, explorerError, explorerSelection,
      selectedAlbumId, albumTracks, albumTracksTruncated, albumDetailState, selectedArtistId,
      selectedTrack, inspectorView, tagSelectionKind, inspectorArtistName, artistDetail,
      artistIntelligence, artistWorldState, artistWorldError,
      loadedKey: loadedExplorerRequestKeyRef.current, viewKey: loadedExplorerViewKeyRef.current,
      localOnly: explorerLocalOnlyRef.current, revision: explorerReloadToken,
    };
  }

  function rememberCurrentView(push: boolean) {
    const view = captureNavigationView();
    exploreRequestRef.current += 1;
    albumRequestRef.current += 1;
    artistRequestRef.current += 1;
    setIsLoadingMore(false);
    explorerLoadingMoreRef.current = false;
    setHistoryLoadingMore(false);
    setReviewLoadingMore(false);
    scrollPositionByDestinationRef.current[view.key] = view.scroll;
    savedViewsRef.current.set(view.key, view);
    workspaceRef.current = { ...workspaceRef.current, scroll: { ...scrollPositionByDestinationRef.current } };
    saveWorkspaceCheckpoint(workspaceRef.current);
    restoringScrollRef.current = true;
    if (push) setNavigationHistory((current) => [...current, view].slice(-50));
  }

  function restoreNavigationView(view: NavigationView) {
    // Invalidate requests from the page being left before restoring its predecessor.
    exploreRequestRef.current += 1;
    albumRequestRef.current += 1;
    artistRequestRef.current += 1;
    setExplorerView(view.explorerView);
    setExplorerFilters(view.explorerFilters);
    setExplorerTracks(view.explorerTracks);
    setExplorerAlbums(view.explorerAlbums);
    setExplorerArtists(view.explorerArtists);
    setExplorerCursor(view.explorerCursor);
    setExplorerCount(view.explorerCount);
    setExplorerLoadState(view.explorerLoadState);
    setExplorerError(view.explorerError);
    setExplorerSelection(view.explorerSelection);
    setSelectedAlbumId(view.selectedAlbumId);
    setAlbumTracks(view.albumTracks);
    setAlbumTracksTruncated(view.albumTracksTruncated);
    setAlbumDetailState(view.albumDetailState === "loading" ? "ready" : view.albumDetailState);
    setSelectedArtistId(view.selectedArtistId);
    setSelectedTrack(view.selectedTrack);
    setInspectorView(view.inspectorView);
    setTagSelectionKind(view.tagSelectionKind);
    setInspectorArtistName(view.inspectorArtistName);
    setArtistDetail(view.artistDetail);
    setArtistIntelligence(view.artistIntelligence);
    setArtistWorldState(view.artistWorldState);
    setArtistWorldError(view.artistWorldError);
    setIsLoadingMore(false);
    explorerLoadingMoreRef.current = false;
    loadedExplorerRequestKeyRef.current = view.explorerLoadState === "ready" && view.albumDetailState !== "loading" ? view.loadedKey : null;
    loadedExplorerViewKeyRef.current = view.viewKey;
    explorerLocalOnlyRef.current = view.localOnly;
    preserveExplorerOnReloadRef.current = view.revision !== explorerReloadToken || view.albumDetailState === "loading";
    scrollPositionByDestinationRef.current[view.key] = view.scroll;
    setNavigationRevision((value) => value + 1);
    setArtistPageName(view.artist);
    setActiveNavState(view.destination);
  }

  function goBack() {
    const previous = navigationHistory[navigationHistory.length - 1];
    if (!previous) return;
    transitionContent(() => {
      rememberCurrentView(false);
      setNavigationHistory((current) => current.slice(0, -1));
      restoreNavigationView(previous);
    }, "page");
  }

  // Used by explicit drilldowns as well as sidebar navigation, including same-page searches.
  function setActiveNav(destination: SidebarDestination) {
    transitionContent(() => {
      rememberCurrentView(true);
      scrollPositionByDestinationRef.current[destination] = 0;
      setNavigationRevision((value) => value + 1);
      setArtistPageName(null);
      setActiveNavState(destination);
    }, "page");
  }

  function navigate(label: SidebarDestination) {
    if (label === activeNav && !artistPageName) return;
    transitionContent(() => {
      rememberCurrentView(true);
      const saved = savedViewsRef.current.get(label);
      if (saved) restoreNavigationView(saved);
      else {
        setNavigationRevision((value) => value + 1);
        setArtistPageName(null);
        setActiveNavState(label);
        const view = explorerViewForDestination(label);
        if (view) {
          changeExplorerView(view);
          setExplorerFilters({ ...defaultExplorerFilters, sort: defaultExplorerSort[view] });
          setSelectedAlbumId(null);
          setExplorerSelection(null);
        }
      }
      if (label !== "Universe" && label !== "Observatory" && label !== "History") expandLibraryNavigation();
    }, "page");
  }

  function openSavedExplorerView(view: ExplorerView, filters: ExplorerFilters) {
    transitionContent(() => {
      rememberCurrentView(true);
      const label = view === "tracks" ? "Songs" : view === "albums" ? "Albums" : "Artists";
      setNavigationRevision(value=>value+1);
      setArtistPageName(null);
      setActiveNavState(label);
      explorer.setExplorerView(view);
      setExplorerFilters({ ...filters });
      setSelectedAlbumId(null);
      setExplorerSelection(null);
      setExplorerCursor(null);
      setExplorerLoadState("loading");
      expandLibraryNavigation();
    }, "page");
  }
  return { rememberCurrentView, restoreNavigationView, goBack, setActiveNav, navigate, openSavedExplorerView };
}
