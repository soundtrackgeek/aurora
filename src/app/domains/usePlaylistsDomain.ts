import { useEffect, useRef, useState } from "react";
import { type Track } from "../../library";
import { usePlayback, type PlaybackSnapshot } from "../../playback";
import { shufflePlaylistTracks } from "../../playlistOrder";
import { listMusicLibraryPlaylists, loadSelectedPlaylistId, saveSelectedPlaylistId, type SavedPlaylistSummary } from "../../playlists";

interface PlaylistsDomainOptions {
  playback: Pick<ReturnType<typeof usePlayback>, "play" | "setShuffle"> & { state: Pick<PlaybackSnapshot, "queue" | "currentIndex" | "shuffle">; };
  appendPlayback: ReturnType<typeof usePlayback>["append"];
  libraryReady: boolean;
  endGenreQueue: () => void;
  selectTrack: (track: Track) => void;
}

/** Owns the playlists destination's state, request guards, and actions. */
export function usePlaylistsDomain({
  playback,
  appendPlayback,
  libraryReady,
  endGenreQueue,
  selectTrack,
}: PlaylistsDomainOptions) {
  const [savedPlaylists, setSavedPlaylists] = useState<SavedPlaylistSummary[]>([]);
  const [selectedPlaylistId, setSelectedPlaylistId] = useState<number | null>(loadSelectedPlaylistId);
  const [playlistsLoading, setPlaylistsLoading] = useState(false);
  const [playlistsError, setPlaylistsError] = useState<string | null>(null);
  const [playlistsReloadToken, setPlaylistsReloadToken] = useState(0);
  const playlistQueueSessionRef = useRef<{ tracks: Track[]; nextIndex: number; queueKeys: string[]; } | null>(null);
  const playlistRefillRunningRef = useRef(false);
  const playlistRefillPromiseRef = useRef<Promise<unknown> | null>(null);

  useEffect(() => {
    const session = playlistQueueSessionRef.current;
    if (!session || playback.state.currentIndex === null || playlistRefillRunningRef.current) return;
    if (playback.state.queue.length !== session.queueKeys.length
      || playback.state.queue.some((track, index) => track.trackKey !== session.queueKeys[index])) {
      playlistQueueSessionRef.current = null;
      return;
    }
    if (session.nextIndex >= session.tracks.length) return;
    if (playback.state.queue.length - playback.state.currentIndex - 1 >= 20) return;
    const nextTracks = session.tracks.slice(session.nextIndex, session.nextIndex + 100);
    playlistRefillRunningRef.current = true;
    const refill = appendPlayback(nextTracks);
    playlistRefillPromiseRef.current = refill;
    void refill.then((next) => {
      if (playlistQueueSessionRef.current !== session) return;
      if (!next) { playlistQueueSessionRef.current = null; return; }
      session.nextIndex += nextTracks.length;
      session.queueKeys = next.queue.map((track) => track.trackKey);
    }).finally(() => {
      playlistRefillRunningRef.current = false;
      if (playlistRefillPromiseRef.current === refill) playlistRefillPromiseRef.current = null;
    });
  }, [playback.state, appendPlayback]);

  useEffect(() => {
    if (!libraryReady) return;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setPlaylistsLoading(true);
      setPlaylistsError(null);
      void listMusicLibraryPlaylists().then((items) => {
        if (cancelled) return;
        setSavedPlaylists(items);
        setSelectedPlaylistId((current) => items.some((item) => item.id === current) ? current : items[0]?.id ?? null);
        setPlaylistsLoading(false);
      }).catch((error: unknown) => {
        if (cancelled) return;
        setPlaylistsError(error instanceof Error ? error.message : String(error));
        setPlaylistsLoading(false);
      });
    }, 0);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [libraryReady, playlistsReloadToken]);

  useEffect(() => {
    if (selectedPlaylistId !== null && savedPlaylists.some((item) => item.id === selectedPlaylistId)) {
      saveSelectedPlaylistId(selectedPlaylistId);
    }
  }, [savedPlaylists, selectedPlaylistId]);

  async function startPlaylistQueue(tracks: Track[], index: number, shuffle = false): Promise<boolean> {
    const remaining = shuffle ? shufflePlaylistTracks(tracks) : tracks.slice(index);
    const first = remaining.slice(0, 100);
    if (first.length === 0) return false;
    playlistQueueSessionRef.current = null;
    if (playlistRefillPromiseRef.current) await playlistRefillPromiseRef.current;
    endGenreQueue();
    if (playback.state.shuffle && !await playback.setShuffle(false)) return false;
    selectTrack(first[0]);
    const next = await playback.play(first, first[0].id);
    if (!next) return false;
    playlistQueueSessionRef.current = {
      tracks: remaining,
      nextIndex: first.length,
      queueKeys: next.queue.map((track) => track.trackKey),
    };
    return true;
  }

  return {
    savedPlaylists,
    selectedPlaylistId,
    setSelectedPlaylistId,
    playlistsLoading,
    playlistsError,
    setPlaylistsReloadToken,
    startPlaylistQueue,
  };
}
