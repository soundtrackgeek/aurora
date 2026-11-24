import { useEffect, useEffectEvent, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { loadChartEntryTrack } from "./charts";
import { isTauriRuntime, loadAlbumDetail, type Track } from "./library";
import { loadMusicLibraryPlaylist, type PlaylistPageContext } from "./playlists";

export type PlaybackLink =
  | { action: "play" | "queue"; kind: "album"; albumId: string }
  | { action: "play" | "queue"; kind: "track"; trackId: string; albumId: string }
  | { action: "play" | "queue"; kind: "playlist"; playlistId: number };

function albumIdValid(id: string | null): id is string {
  return id != null && id.trim().length > 0 && id.length <= 512 && !/\p{Cc}/u.test(id);
}

export function parsePlaybackLink(value: string): PlaybackLink | null {
  if (value.length > 4096) return null;
  try {
    const url = new URL(value);
    const action = url.hostname;
    if (url.protocol !== "aurora:" || (action !== "play" && action !== "queue")
      || url.username || url.password || url.port || url.hash) return null;
    const [, kind, encodedId, extra] = url.pathname.split("/");
    if (!encodedId || extra !== undefined) return null;
    const id = decodeURIComponent(encodedId);
    if (kind === "album" && albumIdValid(id)) return { action, kind, albumId: id };
    if (!/^[1-9]\d{0,15}$/.test(id) || !Number.isSafeInteger(Number(id))) return null;
    if (kind === "playlist") return { action, kind, playlistId: Number(id) };
    const albumId = url.searchParams.get("album");
    if (kind === "track" && albumIdValid(albumId)) return { action, kind, albumId, trackId: id };
  } catch {
    return null;
  }
  return null;
}

type Handlers = {
  play: (tracks: Track[], context?: PlaylistPageContext) => Promise<boolean>;
  queue: (tracks: Track[]) => Promise<unknown>;
};

export async function handlePlaybackLink(link: PlaybackLink, { play, queue }: Handlers) {
  let tracks: Track[];
  let context: PlaylistPageContext | undefined;

  if (link.kind === "track") {
    const track = await loadChartEntryTrack(link.trackId);
    if (track.albumId !== link.albumId) throw new Error("This track has changed in the catalog. Open it again from Music Library.");
    tracks = [track];
  } else if (link.kind === "album") {
    tracks = (await loadAlbumDetail(link.albumId)).tracks;
  } else {
    let page = await loadMusicLibraryPlaylist(link.playlistId);
    tracks = [...page.tracks];
    if (link.action === "play") {
      if (page.revision) context = { id: link.playlistId, cursor: page.nextCursor ?? null, revision: page.revision };
    } else {
      const revision = page.revision ?? null;
      while (page.nextCursor != null && tracks.length <= 200) {
        page = await loadMusicLibraryPlaylist(link.playlistId, page.nextCursor, revision);
        tracks.push(...page.tracks);
      }
      if (tracks.length > 200) throw new Error("Aurora can queue up to 200 songs at once. Use Play for a longer playlist.");
    }
  }

  if (link.kind !== "playlist") tracks = tracks.filter(track => track.loveState !== "banned");
  if (!tracks.length) throw new Error("No playable tracks were found in the shared catalog.");
  if (link.action === "play") return play(tracks, context);
  return Boolean(await queue(tracks));
}

export function useMusicLibraryPlaybackLinks(handlers: Handlers, onError: (message: string) => void) {
  const seen = useRef(new Set<string>());
  const execute = useEffectEvent((link: PlaybackLink) => handlePlaybackLink(link, handlers));
  const report = useEffectEvent((reason: unknown) => {
    onError(reason instanceof Error ? reason.message : String(reason));
  });

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let closed = false;
    let release: (() => void) | undefined;
    let pending = Promise.resolve();
    const handled = seen.current;

    function receive(urls: string[]) {
      for (const url of urls) {
        const link = parsePlaybackLink(url);
        if (!link || handled.has(url)) continue;
        handled.add(url);
        if (handled.size > 64) handled.delete(handled.values().next().value!);
        pending = pending.then(async () => {
          if (!closed) await execute(link);
        }).catch((reason: unknown) => { if (!closed) report(reason); });
      }
    }

    void onOpenUrl(receive).then(async unlisten => {
      if (closed) {
        unlisten();
        return;
      }
      release = unlisten;
      const initial = await getCurrent();
      if (!closed && initial) receive(initial);
    }).catch((reason: unknown) => { if (!closed) report(reason); });

    return () => {
      closed = true;
      release?.();
    };
  }, []);
}

export function openMusicLibraryAlbum(albumId: string) {
  if (!isTauriRuntime()) return Promise.reject(new Error("Open in Music Library is available in the Aurora desktop app."));
  return invoke<void>("open_music_library_album", { albumId });
}
