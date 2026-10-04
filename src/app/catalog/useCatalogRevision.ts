import { useCallback, useEffect, useRef, type RefObject } from "react";
import {
  catalogRefreshIsConsistent,
  loadCatalogRevision,
  loadLibrarySnapshot,
  type LibrarySnapshot,
} from "../../library";
import type { PlaybackCatalogRebind, PlaybackSnapshot } from "../../playback";

export interface CatalogRevisionOptions {
  libraryReady: boolean;
  latestTagProjectionTokenRef: RefObject<number>;
  rebindPlaybackCatalog: () => Promise<PlaybackCatalogRebind | null>;
  onCatalogRefresh: (snapshot: LibrarySnapshot, playback: PlaybackSnapshot) => void;
}

/** Refreshes only mutually consistent catalog/playback snapshots, once at a time. */
export function useCatalogRevision({
  libraryReady,
  latestTagProjectionTokenRef,
  rebindPlaybackCatalog,
  onCatalogRefresh,
}: CatalogRevisionOptions) {
  const catalogRevisionRef = useRef<string | null>(null);
  const catalogRefreshPromiseRef = useRef<Promise<boolean> | null>(null);
  const catalogRefreshRequestedRef = useRef(false);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);

  const refreshCatalogIfChanged = useCallback((
    isCurrent: () => boolean = () => true,
  ): Promise<boolean> => {
    const shouldContinue = () => mountedRef.current && isCurrent();
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

        onCatalogRefresh(nextSnapshot, reboundResult.playback);
        catalogRevisionRef.current = revision;
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
  }, [latestTagProjectionTokenRef, onCatalogRefresh, rebindPlaybackCatalog]);

  useEffect(() => {
    if (!libraryReady) return;
    let cancelled = false;
    const refreshQuietly = () => {
      void refreshCatalogIfChanged(() => !cancelled).catch((error: unknown) => {
        console.warn("Aurora could not check the Music Library catalog revision", error);
      });
    };
    const initialRefresh = window.setTimeout(refreshQuietly, 0);
    const interval = window.setInterval(refreshQuietly, 5_000);
    window.addEventListener("focus", refreshQuietly);
    return () => {
      cancelled = true;
      window.clearTimeout(initialRefresh);
      window.clearInterval(interval);
      window.removeEventListener("focus", refreshQuietly);
    };
  }, [libraryReady, refreshCatalogIfChanged]);

  return { catalogRevisionRef, refreshCatalogIfChanged };
}
