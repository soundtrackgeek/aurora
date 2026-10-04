import { useCallback, useEffect, useRef, useState } from "react";
import { formatCount } from "../../library";
import {
  advanceCatalogProjectionToken,
  reconcilePendingTags,
  retryPendingLibrarySync,
  type CatalogSync,
  type CatalogTrackProjectionDecision,
  type TagReconciliationChange,
} from "../../tags";
import { catalogSyncNeedsRetry } from "./catalogSyncNotice";

const retryIntervalMs = 5_000;

export interface PendingLibrarySyncOptions {
  libraryReady: boolean;
  reloadToken: number;
  refreshCatalogIfChanged: () => Promise<boolean>;
  acceptTrackProjectionKeys: (
    trackKeys: readonly string[], projectionToken: number | null | undefined,
  ) => CatalogTrackProjectionDecision;
  onReconciliationChanges: (changes: TagReconciliationChange[]) => void;
  onChartsChanged: () => void;
  setSyncMessage: (message: string | null) => void;
}

/** Owns pending tag reconciliation, retry scheduling, and the settled sync notice. */
export function usePendingLibrarySync({
  libraryReady,
  reloadToken,
  refreshCatalogIfChanged,
  acceptTrackProjectionKeys,
  onReconciliationChanges,
  onChartsChanged,
  setSyncMessage,
}: PendingLibrarySyncOptions) {
  const [catalogSyncNotice, setCatalogSyncNotice] = useState<CatalogSync | null>(null);
  const [reconciliationHasMore, setReconciliationHasMore] = useState(false);
  const catalogSyncNoticeRef = useRef<CatalogSync | null>(null);
  const latestCatalogSyncTokenRef = useRef(0);
  const reconciliationRunningRef = useRef(false);
  const librarySyncRunningRef = useRef(false);
  const appFocusedRef = useRef(typeof document === "undefined" ? true : document.hasFocus());
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);

  const handleCatalogSync = useCallback(async (
    sync: CatalogSync | null | undefined,
    announceSuccess = false,
  ): Promise<boolean> => {
    if (!sync || !mountedRef.current) return false;
    const previousSyncToken = latestCatalogSyncTokenRef.current;
    const syncDecision = advanceCatalogProjectionToken(previousSyncToken, sync.projectionToken);
    if (!syncDecision.accepted) return false;
    latestCatalogSyncTokenRef.current = syncDecision.latestToken;
    const previous = catalogSyncNoticeRef.current;
    const wasUnsettled = previous?.status === "pending" || previous?.status === "blocked";
    const needsRetry = catalogSyncNeedsRetry(sync);
    if (needsRetry || sync.status === "blocked" || announceSuccess || wasUnsettled) {
      catalogSyncNoticeRef.current = sync;
      setCatalogSyncNotice(sync);
    } else if (!previous) {
      catalogSyncNoticeRef.current = sync;
    }
    try {
      const refreshed = await refreshCatalogIfChanged();
      // Tag synchronization need not create an import run. Retained Charts still
      // need to refresh when the import-based catalog revision has not changed.
      if (mountedRef.current && !refreshed && sync.status === "synced"
        && (announceSuccess || wasUnsettled || syncDecision.latestToken !== previousSyncToken)) {
        onChartsChanged();
      }
      return refreshed;
    } catch (error) {
      console.warn("Aurora could not check Music Library for partial sync updates yet", error);
      return false;
    }
  }, [onChartsChanged, refreshCatalogIfChanged]);

  const refreshExternalTagChanges = useCallback(async () => {
    if (!mountedRef.current || reconciliationRunningRef.current) return;
    reconciliationRunningRef.current = true;
    try {
      const report = await reconcilePendingTags();
      if (!mountedRef.current) return;
      setReconciliationHasMore(report.hasMore);
      const projection = acceptTrackProjectionKeys(
        report.changes.map((change) => change.trackKey),
        report.projectionToken,
      );
      onReconciliationChanges(report.changes.filter((change) => (
        projection.acceptedTrackKeys.has(change.trackKey)
      )));
      if (report.externalChanges > 0) {
        setSyncMessage(`Refreshed ${formatCount(report.externalChanges)} external tag ${report.externalChanges === 1 ? "change" : "changes"}`);
      } else if (report.issues.length > 0 || report.hasMore) {
        setSyncMessage(report.issues.length === 1 && report.issues[0]?.message
          ? report.issues[0].message
          : `${formatCount(report.issues.length)} tag ${report.issues.length === 1 ? "item needs" : "items need"} attention`);
      } else if (!catalogSyncNeedsRetry(catalogSyncNoticeRef.current)) {
        setSyncMessage(null);
      }
    } catch (error) {
      if (!mountedRef.current) return;
      console.warn("Aurora could not reconcile pending tags", error);
      setReconciliationHasMore(true);
      setSyncMessage("Tag and Music Library refresh will retry automatically");
    } finally {
      reconciliationRunningRef.current = false;
    }
  }, [acceptTrackProjectionKeys, onReconciliationChanges, setSyncMessage]);

  const retryPendingLibrarySyncNow = useCallback(async () => {
    if (!mountedRef.current || librarySyncRunningRef.current) return;
    librarySyncRunningRef.current = true;
    try {
      await handleCatalogSync(await retryPendingLibrarySync());
    } catch (error) {
      console.warn("Aurora could not retry the pending Music Library update", error);
    } finally {
      librarySyncRunningRef.current = false;
    }
  }, [handleCatalogSync]);

  useEffect(() => {
    if (!libraryReady) return;
    const initialRefresh = window.setTimeout(() => void refreshExternalTagChanges(), 0);
    const refreshOnFocus = () => {
      appFocusedRef.current = true;
      void refreshExternalTagChanges();
    };
    const pauseOnBlur = () => { appFocusedRef.current = false; };
    appFocusedRef.current = document.hasFocus();
    window.addEventListener("focus", refreshOnFocus);
    window.addEventListener("blur", pauseOnBlur);
    return () => {
      window.clearTimeout(initialRefresh);
      window.removeEventListener("focus", refreshOnFocus);
      window.removeEventListener("blur", pauseOnBlur);
    };
  }, [libraryReady, reloadToken, refreshExternalTagChanges]);

  useEffect(() => {
    if (!libraryReady || !reconciliationHasMore) return;
    const interval = window.setInterval(() => {
      if (appFocusedRef.current) void refreshExternalTagChanges();
    }, retryIntervalMs);
    return () => window.clearInterval(interval);
  }, [libraryReady, reconciliationHasMore, refreshExternalTagChanges]);

  useEffect(() => {
    if (!libraryReady) return;
    const initialRetry = window.setTimeout(() => void retryPendingLibrarySyncNow(), 0);
    return () => window.clearTimeout(initialRetry);
  }, [libraryReady, reloadToken, retryPendingLibrarySyncNow]);

  useEffect(() => {
    if (!libraryReady || !catalogSyncNeedsRetry(catalogSyncNotice)) return;
    const interval = window.setInterval(() => { void retryPendingLibrarySyncNow(); }, retryIntervalMs);
    return () => window.clearInterval(interval);
  }, [catalogSyncNotice, libraryReady, retryPendingLibrarySyncNow]);

  useEffect(() => {
    if (catalogSyncNotice?.status !== "synced") return;
    const settledNotice = catalogSyncNotice;
    const timeout = window.setTimeout(() => {
      if (catalogSyncNoticeRef.current !== settledNotice) return;
      catalogSyncNoticeRef.current = null;
      setCatalogSyncNotice(null);
    }, 6_000);
    return () => window.clearTimeout(timeout);
  }, [catalogSyncNotice]);

  return { catalogSyncNotice, handleCatalogSync, refreshExternalTagChanges, retryPendingLibrarySyncNow };
}
