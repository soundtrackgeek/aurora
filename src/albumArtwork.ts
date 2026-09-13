import { useSyncExternalStore } from "react";
import { albumCoverUrl } from "./library";

const revisions = new Map<string, number>();
// Protocol responses are immutable in the webview cache. A new app session must
// recheck the source; native thumbnails still reuse their file fingerprint cache.
const sessionRevision = Date.now().toString(36);
const listeners = new Set<() => void>();
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
};

export function refreshAlbumArtwork(albumId: string): void {
  revisions.set(albumId, (revisions.get(albumId) ?? 0) + 1);
  listeners.forEach((listener) => listener());
}

export function useAlbumCoverUrl(albumId: string | null, size: 64 | 128 | 256 | 512): string | null {
  const revision = useSyncExternalStore(subscribe, () => albumId ? revisions.get(albumId) ?? 0 : 0);
  const source = albumCoverUrl(albumId, size);
  return source ? `${source}&revision=${sessionRevision}-${revision}` : null;
}
