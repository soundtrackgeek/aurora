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
): { preservingCurrentView: boolean; pending: boolean } {
  if (!explorerActive) {
    return { preservingCurrentView: false, pending };
  }

  return { preservingCurrentView: pending, pending: false };
}
