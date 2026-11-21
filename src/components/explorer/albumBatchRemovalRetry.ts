/** Waits before retrying an album whose removal met a busy catalog; four attempts in all. */
export const BUSY_RETRY_DELAYS_MS: readonly number[] = [5_000, 15_000, 30_000];

/**
 * Music Library refuses a removal before its catalog transaction when another connection keeps
 * the database busy, for example while it settles the previous album's commit. The copy is then
 * verified and removed and the source remains, so a fresh preview and apply is safe.
 */
export function isLibraryBusyFailure(message: string): boolean {
  return /database is locked by another operation|database is still busy|catalog connections are busy|catalog changed while its recovery backup was being prepared/i.test(message);
}
