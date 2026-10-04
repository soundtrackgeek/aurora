import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import * as tags from "../../tags";
import { useCatalogProjection } from "./useCatalogProjection";
import { usePendingLibrarySync } from "./usePendingLibrarySync";

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.useRealTimers(); });

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((finish) => { resolve = finish; });
  return { promise, resolve };
}

function report(changes: tags.TagReconciliationChange[] = []): tags.TagReconciliationReport {
  return {
    projectionToken: 9, processed: changes.length, reconciled: 0, externalChanges: changes.length,
    catalogCaughtUp: 0, unchanged: 0, unavailable: 0, invalid: 0, conflicted: 0,
    hasMore: false, changes, issues: [],
  };
}

function options() {
  return {
    libraryReady: false,
    reloadToken: 0,
    refreshCatalogIfChanged: vi.fn(async () => false),
    onReconciliationChanges: vi.fn(),
    onChartsChanged: vi.fn(),
    setSyncMessage: vi.fn(),
  };
}

it("filters stale reconciliation changes per track while retaining unrelated updates", async () => {
  const input = options();
  const changes = ["edited", "other"].map((trackKey): tags.TagReconciliationChange => ({
    trackKey, values: { rating: 2, loveState: "neutral", releaseYear: 2000 }, syncState: null,
  }));
  const pending = deferred<tags.TagReconciliationReport>();
  vi.spyOn(tags, "reconcilePendingTags").mockReturnValue(pending.promise);
  const { result } = renderHook(() => {
    const projection = useCatalogProjection();
    return { ...projection, ...usePendingLibrarySync({ ...input, acceptTrackProjectionKeys: projection.acceptTrackProjectionKeys }) };
  });
  const reconcile = result.current.refreshExternalTagChanges();
  result.current.acceptTrackProjectionKeys(["edited"], 10);
  await act(async () => { pending.resolve(report(changes)); await reconcile; });
  expect(input.onReconciliationChanges).toHaveBeenCalledWith([changes[1]]);
  expect(result.current.latestTagProjectionTokenRef.current).toBe(10);
});

it("serializes library retries and ignores stale sync tokens", async () => {
  const input = options();
  const pending = deferred<tags.CatalogSync>();
  const retry = vi.spyOn(tags, "retryPendingLibrarySync").mockReturnValue(pending.promise);
  const { result } = renderHook(() => {
    const projection = useCatalogProjection();
    return usePendingLibrarySync({ ...input, acceptTrackProjectionKeys: projection.acceptTrackProjectionKeys });
  });
  const first = result.current.retryPendingLibrarySyncNow();
  await result.current.retryPendingLibrarySyncNow();
  expect(retry).toHaveBeenCalledTimes(1);
  await act(async () => {
    await result.current.handleCatalogSync({ status: "synced", pendingFolderCount: 0, projectionToken: 10 }, true);
    pending.resolve({ status: "pending", pendingFolderCount: 1, projectionToken: 9 });
    await first;
  });
  expect(result.current.catalogSyncNotice?.status).toBe("synced");
  expect(input.refreshCatalogIfChanged).toHaveBeenCalledTimes(1);
  expect(input.onChartsChanged).toHaveBeenCalledTimes(1);
});

it("retries pending syncs, stops blocked syncs, and removes timers on unmount", async () => {
  vi.useFakeTimers();
  const input = { ...options(), libraryReady: true };
  vi.spyOn(tags, "reconcilePendingTags").mockResolvedValue(report());
  const retry = vi.spyOn(tags, "retryPendingLibrarySync").mockResolvedValue({ status: "pending", pendingFolderCount: 1 });
  const { result, unmount } = renderHook(() => {
    const projection = useCatalogProjection();
    return usePendingLibrarySync({ ...input, acceptTrackProjectionKeys: projection.acceptTrackProjectionKeys });
  });
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  expect(retry).toHaveBeenCalledTimes(1);
  await act(async () => { await vi.advanceTimersByTimeAsync(5_000); });
  expect(retry).toHaveBeenCalledTimes(2);
  await act(async () => { await result.current.handleCatalogSync({ status: "blocked", pendingFolderCount: 1 }); });
  await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
  expect(retry).toHaveBeenCalledTimes(2);
  unmount();
  expect(vi.getTimerCount()).toBe(0);
});

it("discards reconciliation results after unmount", async () => {
  const input = options();
  const pending = deferred<tags.TagReconciliationReport>();
  vi.spyOn(tags, "reconcilePendingTags").mockReturnValue(pending.promise);
  const { result, unmount } = renderHook(() => {
    const projection = useCatalogProjection();
    return usePendingLibrarySync({ ...input, acceptTrackProjectionKeys: projection.acceptTrackProjectionKeys });
  });
  const request = result.current.refreshExternalTagChanges();
  unmount();
  pending.resolve(report());
  await request;
  expect(input.onReconciliationChanges).not.toHaveBeenCalled();
  expect(input.setSyncMessage).not.toHaveBeenCalled();
});
