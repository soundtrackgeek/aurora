import type { InboxAlbum } from "./inbox";

export interface InboxAlbumIdentityChange {
  from: string;
  to: string;
}

// A scan is a complete snapshot sorted by modification time. Keep the working
// order so saving tags does not move a row (and the browser's scroll anchor).
export function mergeRefreshedInboxAlbums(
  current: readonly InboxAlbum[],
  refreshed: readonly InboxAlbum[],
  renamed?: InboxAlbumIdentityChange,
): InboxAlbum[] {
  const remaining = new Map(refreshed.map((album) => [album.id, album]));
  const retained = current.flatMap((album) => {
    const id = album.id === renamed?.from ? renamed.to : album.id;
    const updated = remaining.get(id);
    remaining.delete(id);
    return updated ? [updated] : [];
  });
  return [...retained, ...remaining.values()];
}
