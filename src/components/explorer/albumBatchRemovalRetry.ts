/** Waits before retrying an album whose removal met a busy catalog; five attempts in all. */
export const BUSY_RETRY_DELAYS_MS: readonly number[] = [5_000, 15_000, 30_000, 60_000];

/**
 * Music Library refuses a preview or removal when another process keeps the catalog busy, for
 * example its background maintenance right after the previous album's commit. A busy catalog
 * rolls the work back: the verified copy is removed and the source remains. Every retry starts
 * with a fresh preview, which checks the album again before anything moves.
 */
export function isLibraryBusyFailure(message: string): boolean {
  return /database is locked|database file is locked|database is still busy|\(databaseBusy\)|catalog connections are busy|catalog changed while its recovery backup was being prepared|another aurora album intake is already running/i.test(message);
}
