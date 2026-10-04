import { useCallback, useEffect, useRef, useState } from "react";
import { subscribeNativeEvent } from "../../nativeEvents";
import { formatCount } from "../../library";
import {
  advanceCatalogProjectionToken,
  reconcilePendingTags,
  loadLibrarySyncStatus,
  retryPendingLibrarySync,
  type CatalogSync,
  type CatalogTrackProjectionDecision,
  type TagReconciliationChange,
  type TagReconciliationReport,
} from "../../tags";
import { catalogSyncNeedsRetry } from "./catalogSyncNotice";

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

/** Projects native tag/sync events and owns the settled sync notice. */
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
  const catalogSyncNoticeRef = useRef<CatalogSync | null>(null);
  const latestCatalogSyncTokenRef = useRef(0);
  const reconciliationRunningRef = useRef(false);
  const librarySyncRunningRef = useRef(false);
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

  const applyReconciliation = useCallback((report: TagReconciliationReport) => {
    if (!mountedRef.current) return;
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
  }, [acceptTrackProjectionKeys, onReconciliationChanges, setSyncMessage]);

  const refreshExternalTagChanges = useCallback(async () => {
    if (!mountedRef.current || reconciliationRunningRef.current) return;
    reconciliationRunningRef.current = true;
    try {
      const report = await reconcilePendingTags();
      applyReconciliation(report);
    } catch (error) {
      if (!mountedRef.current) return;
      console.warn("Aurora could not reconcile pending tags", error);
      setSyncMessage("Tag and Music Library refresh will retry automatically");
    } finally {
      reconciliationRunningRef.current = false;
    }
  }, [applyReconciliation, setSyncMessage]);

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
    let initial: number | undefined;
    const stop = subscribeNativeEvent<TagReconciliationReport>("tags://reconciled", applyReconciliation, () => {
      initial = window.setTimeout(() => void refreshExternalTagChanges(), 0);
    });
    const refreshOnFocus = () => void refreshExternalTagChanges();
    window.addEventListener("focus", refreshOnFocus);
    return () => {
      stop();
      window.clearTimeout(initial);
      window.removeEventListener("focus", refreshOnFocus);
    };
  }, [libraryReady, reloadToken, applyReconciliation, refreshExternalTagChanges]);

  useEffect(() => {
    if (!libraryReady) return;
    let initial: number | undefined;
    const stop = subscribeNativeEvent<CatalogSync>("library-sync://status", (sync) => {
      void handleCatalogSync(sync);
    }, () => {
      initial = window.setTimeout(() => {
        void loadLibrarySyncStatus().then((sync) => handleCatalogSync(sync)).catch((error: unknown) => {
          console.warn("Aurora could not load Music Library sync status", error);
        });
      }, 0);
    });
    return () => { stop(); window.clearTimeout(initial); };
  }, [libraryReady, reloadToken, handleCatalogSync]);

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
