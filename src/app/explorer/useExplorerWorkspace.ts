import { useLayoutEffect, useRef, useState } from "react";
import type { ExplorerFilters, ExplorerView, ExplorerSelection, ExplorerLoadState } from "../../components/explorer/DeepExplorer";
import type { Track, AlbumSummary, Artist, ExplorerCursor } from "../../library";
import type { ViewPreferences } from "../../viewPreferences";

/** Mutable explorer state shared with selection, catalog projection, and retained navigation. */
export function useExplorerWorkspace(initialViewPreferences: ViewPreferences) {
  const [explorerView, setExplorerView] = useState<ExplorerView>(initialViewPreferences.explorerView);
  const [explorerFilters, setExplorerFilters] = useState<ExplorerFilters>(initialViewPreferences.explorerFilters);
  const [explorerTracks, setExplorerTracks] = useState<Track[]>([]);
  const [explorerAlbums, setExplorerAlbums] = useState<AlbumSummary[]>([]);
  const [explorerArtists, setExplorerArtists] = useState<Artist[]>([]);
  const [explorerCursor, setExplorerCursor] = useState<ExplorerCursor | null>(null);
  const [explorerCount, setExplorerCount] = useState<{ key: string; total: number; } | null>(null);
  const [explorerLoadState, setExplorerLoadState] = useState<ExplorerLoadState>("loading");
  const [explorerError, setExplorerError] = useState<string | null>(null);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [explorerReloadToken, setExplorerReloadToken] = useState(0);
  const [explorerSelection, setExplorerSelection] = useState<ExplorerSelection | null>(null);
  const exploreRequestRef = useRef(0);
  const loadedExplorerRequestKeyRef = useRef<string | null>(null);
  const loadedExplorerViewKeyRef = useRef<string | null>(null);
  const explorerCursorRef = useRef<ExplorerCursor | null>(null);
  const explorerLoadedRef = useRef(0);
  const explorerLocalOnlyRef = useRef(true);
  const explorerLoadingMoreRef = useRef(false);
  const explorerRestorationPendingRef = useRef(true);
  const preserveExplorerOnReloadRef = useRef(false);
  const pendingExplorerAlbumIdRef = useRef<string | null>(null);

  useLayoutEffect(() => {
    explorerCursorRef.current = explorerCursor;
    explorerLoadedRef.current = explorerTracks.length + explorerAlbums.length + explorerArtists.length;
  }, [explorerCursor, explorerTracks.length, explorerAlbums.length, explorerArtists.length]);

  return {
    explorerView,
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
    isLoadingMore,
    setIsLoadingMore,
    explorerReloadToken,
    setExplorerReloadToken,
    explorerSelection,
    setExplorerSelection,
    exploreRequestRef,
    loadedExplorerRequestKeyRef,
    loadedExplorerViewKeyRef,
    explorerCursorRef,
    explorerLoadedRef,
    explorerLocalOnlyRef,
    explorerLoadingMoreRef,
    explorerRestorationPendingRef,
    preserveExplorerOnReloadRef,
    pendingExplorerAlbumIdRef
  };
}

export type ExplorerWorkspace = ReturnType<typeof useExplorerWorkspace>;
