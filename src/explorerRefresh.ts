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
