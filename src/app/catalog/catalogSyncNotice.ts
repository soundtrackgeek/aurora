import { formatCount } from "../../library";
import type { CatalogSync } from "../../tags";

export function catalogSyncNeedsRetry(sync: CatalogSync | null): boolean {
  return sync?.status === "pending";
}

export function catalogSyncMessage(sync: CatalogSync): string {
  if (sync.status === "synced" && sync.pendingFolderCount > 0) {
    return `Music Library updated this edit · ${formatCount(sync.pendingFolderCount)} other ${sync.pendingFolderCount === 1 ? "folder is" : "folders are"} pending; retrying automatically`;
  }
  if (sync.status === "pending") {
    return `MP3 changes are saved · ${sync.message?.trim() || "Music Library update pending; retrying automatically."}`;
  }
  if (sync.status === "blocked") {
    return `MP3 changes are saved · ${sync.message?.trim() || "Music Library update needs attention; automatic retries are paused."}`;
  }
  return sync.message?.trim() || "Music Library updated.";
}
