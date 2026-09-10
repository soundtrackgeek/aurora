export function mergeRefreshedExplorerPage<T extends { id: string }>(
  current: readonly T[],
  refreshed: readonly T[],
): T[] {
  const refreshedById = new Map(refreshed.map((item) => [item.id, item]));
  const currentIds = new Set(current.map((item) => item.id));
  return [
    ...current.map((item) => refreshedById.get(item.id) ?? item),
    ...refreshed.filter((item) => !currentIds.has(item.id)),
  ];
}

export function refreshedExplorerCursor<T>(
  currentLoaded: number,
  refreshedLoaded: number,
  currentCursor: T | null,
  refreshedCursor: T | null,
): T | null {
  return currentLoaded > refreshedLoaded ? currentCursor : refreshedCursor;
}

export function shouldReuseExplorerPage(
  loadedRequestKey: string | null,
  currentRequestKey: string,
  preservingCurrentView: boolean,
): boolean {
  return !preservingCurrentView && loadedRequestKey === currentRequestKey;
}

export function resolveExplorerRefreshPreservation(
  pending: boolean,
  explorerActive: boolean,
  matchesLoadedView: boolean,
): { preservingCurrentView: boolean; pending: boolean } {
  if (!explorerActive) {
    return { preservingCurrentView: false, pending };
  }

  return { preservingCurrentView: pending && matchesLoadedView, pending: false };
}

interface ExplorerRefreshJob<T> {
  load: () => Promise<T>;
  cancelled: () => boolean;
  apply: (value: T) => void;
  failed: (error: unknown) => void;
}

// Keep slow share reconciliation out of the foreground, with at most one worker
// and one pending replacement. New searches/pagination supersede obsolete work.
export function createExplorerRefreshQueue<T>() {
  let running = false;
  let pending: ExplorerRefreshJob<T> | null = null;
  async function drain() {
    if (running) return;
    running = true;
    try {
      while (pending) {
        const job = pending;
        pending = null;
        if (job.cancelled()) continue;
        try {
          const result = await job.load();
          if (!job.cancelled()) job.apply(result);
        } catch (error) {
          if (!job.cancelled()) job.failed(error);
        }
      }
    } finally {
      running = false;
    }
  }
  return (job: ExplorerRefreshJob<T>) => {
    pending = job;
    void drain();
  };
}
