import { useEffect, useLayoutEffect, useRef, useState, type UIEvent } from "react";
import type { Track } from "../../library";
import { loadWorkspaceCheckpoint, saveWorkspaceCheckpoint, restoreWorkspaceScroll } from "../../workspaceRestoration";
import { explorerRequestKey } from "../explorer/explorerQueries";
import type { ExplorerWorkspace } from "../explorer/useExplorerWorkspace";

/** Durable checkpoint and the scroll containers shared by navigation and explorer restoration. */
export function useWorkspaceRestoration() {
  const [initialWorkspace] = useState(loadWorkspaceCheckpoint);
  const workspaceRef = useRef(initialWorkspace);
  const restoringScrollRef = useRef(false);
  const scrollReadyRef = useRef(false);
  const mainScrollRef = useRef<HTMLDivElement>(null);
  const scrollPositionByDestinationRef = useRef<Record<string, number>>(initialWorkspace.scroll);

  function rememberScroll(event: UIEvent<HTMLDivElement>) {
    if (restoringScrollRef.current) return;
    const pageKey = event.currentTarget.dataset.pageKey;
    if (!pageKey) return;
    const nextScroll = event.currentTarget.scrollTop;
    if (nextScroll === 0 && (scrollPositionByDestinationRef.current[pageKey] ?? 0) > 0 && !scrollReadyRef.current) return;
    scrollPositionByDestinationRef.current[pageKey] = nextScroll;
    workspaceRef.current = { ...workspaceRef.current, scroll: { ...scrollPositionByDestinationRef.current } };
    saveWorkspaceCheckpoint(workspaceRef.current);
  }

  return { initialWorkspace, workspaceRef, restoringScrollRef, scrollReadyRef, mainScrollRef, scrollPositionByDestinationRef, rememberScroll };
}

export type WorkspaceRestoration = ReturnType<typeof useWorkspaceRestoration>;

interface WorkspaceCheckpointOptions {
  workspace: WorkspaceRestoration;
  explorer: ExplorerWorkspace;
  selectedTrack: Track | null;
  pageKey: string;
  artistPageName: string | null;
  navigationRevision: number;
}

function hasVisibleRouteFallback(container: HTMLElement): boolean {
  return Array.from(container.querySelectorAll<HTMLElement>("[data-route-loading]")).some((fallback) => {
    // Activity can hide the fallback itself or any enclosing destination node.
    // Retained hidden pages must not delay the visible destination's restoration.
    for (let element: HTMLElement | null = fallback; element && element !== container; element = element.parentElement) {
      if (element.hidden || element.style.display === "none") return false;
    }
    return true;
  });
}

/** Restore only after the active destination and any expanded album have committed. */
export function useWorkspaceCheckpoint({ workspace, explorer, selectedTrack, pageKey, artistPageName, navigationRevision }: WorkspaceCheckpointOptions) {
  const { workspaceRef, mainScrollRef, restoringScrollRef, scrollPositionByDestinationRef, scrollReadyRef } = workspace;
  const {
    explorerLoadState,
    explorerRestorationPendingRef,
    loadedExplorerRequestKeyRef,
    explorerView,
    explorerFilters,
    explorerReloadToken,
    explorerTracks,
    explorerAlbums,
    explorerArtists
  } = explorer;
  useEffect(() => {
    if (explorerLoadState !== "ready" || explorerRestorationPendingRef.current
      || loadedExplorerRequestKeyRef.current !== explorerRequestKey(explorerView, explorerFilters, explorerReloadToken)) return;
    workspaceRef.current = {
      ...workspaceRef.current,
      explorerKey: explorerRequestKey(explorerView, explorerFilters, 0),
      loaded: explorerTracks.length + explorerAlbums.length + explorerArtists.length,
      trackKey: selectedTrack?.trackKey ?? null,
    };
    saveWorkspaceCheckpoint(workspaceRef.current);
  }, [explorerLoadState, explorerView, explorerFilters, explorerReloadToken, explorerTracks.length, explorerAlbums.length, explorerArtists.length, selectedTrack, explorerRestorationPendingRef, loadedExplorerRequestKeyRef, workspaceRef]);

  useLayoutEffect(() => {
    const scrollContainer = mainScrollRef.current;
    if (!scrollContainer) return undefined;
    restoringScrollRef.current = true;
    return restoreWorkspaceScroll(scrollContainer, scrollPositionByDestinationRef.current[pageKey] ?? 0,
      () => scrollReadyRef.current
        && !hasVisibleRouteFallback(scrollContainer)
        && !scrollContainer.querySelector('.chart-studio[aria-busy="true"]:not([style*="display: none"])')
        && (artistPageName ? !!scrollContainer.querySelector('.artist-page:not([style*="display: none"])') : true),
      () => { restoringScrollRef.current = false; });
  }, [pageKey, artistPageName, navigationRevision, mainScrollRef, restoringScrollRef, scrollPositionByDestinationRef, scrollReadyRef]);

}
