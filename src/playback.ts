import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { previewAudioSnapshot, type ReplayGainMode } from "./audio";
import { subscribeNativeEvent } from "./nativeEvents";
import {
  applyEditableTrackTagProjection,
  applyTrackTagProjection,
  isTauriRuntime,
  type Track,
} from "./library";

export type PlaybackStatus = "stopped" | "playing" | "paused" | "error";
export type RepeatMode = "off" | "all" | "one";

export interface PlaybackSnapshot {
  eventSequence?: number;
  queue: Track[];
  currentIndex: number | null;
  currentTrack: Track | null;
  status: PlaybackStatus;
  positionSeconds: number;
  volume: number;
  shuffle: boolean;
  repeatMode: RepeatMode;
  error: string | null;
  outputDeviceLabel: string | null;
  usingDeviceFallback: boolean;
  replayGainMode: ReplayGainMode;
  replayGainDb: number | null;
  replayGainSource: ReplayGainMode | null;
  clippingPrevented: boolean;
  audioUnderrunCount: number;
  realtimeSchedulingDenied: boolean;
}

export interface PlaybackCatalogRebind {
  playback: PlaybackSnapshot;
  catalogRevision: string;
}

const emptyPlayback: PlaybackSnapshot = {
  queue: [],
  currentIndex: null,
  currentTrack: null,
  status: "stopped",
  positionSeconds: 0,
  volume: 0.7,
  shuffle: false,
  repeatMode: "off",
  error: null,
  outputDeviceLabel: "Speakers (Realtek Audio)",
  usingDeviceFallback: false,
  replayGainMode: "off",
  replayGainDb: null,
  replayGainSource: null,
  clippingPrevented: false,
  audioUnderrunCount: 0,
  realtimeSchedulingDenied: false,
};

let browserPlayback: PlaybackSnapshot = { ...emptyPlayback };
let browserStartedAt = 0;
let browserPriorityEnd: number | null = null;

function cloneBrowserPlayback(): PlaybackSnapshot {
  return {
    ...browserPlayback,
    queue: [...browserPlayback.queue],
    currentTrack: browserPlayback.currentTrack ? { ...browserPlayback.currentTrack } : null,
  };
}

function chooseBrowserNext(): number | null {
  const { currentIndex, queue, repeatMode, shuffle } = browserPlayback;
  if (currentIndex === null || queue.length === 0) return null;
  if (browserPriorityEnd !== null) {
    if (currentIndex < browserPriorityEnd) return currentIndex + 1;
    browserPriorityEnd = null;
  }
  if (repeatMode === "one") return currentIndex;
  if (shuffle && queue.length > 1) return (currentIndex + 3) % queue.length;
  if (currentIndex + 1 < queue.length) return currentIndex + 1;
  return repeatMode === "all" ? 0 : null;
}

function refreshBrowserClock(): void {
  const audio = previewAudioSnapshot();
  const replayGainMode = audio.settings.replayGainMode;
  browserPlayback = {
    ...browserPlayback,
    outputDeviceLabel: audio.activeDeviceLabel,
    usingDeviceFallback: audio.usingFallback,
    replayGainMode,
    replayGainDb: replayGainMode === "off" || !browserPlayback.currentTrack
      ? null
      : replayGainMode === "album" ? -8.1 : -6.4,
    replayGainSource: replayGainMode === "off" || !browserPlayback.currentTrack
      ? null
      : replayGainMode,
    clippingPrevented: false,
  };
  if (browserPlayback.status !== "playing" || browserPlayback.currentIndex === null) return;
  const elapsed = Math.max(0, (performance.now() - browserStartedAt) / 1000);
  const duration = browserPlayback.currentTrack?.durationSeconds ?? Number.POSITIVE_INFINITY;
  if (elapsed < duration) {
    browserPlayback = { ...browserPlayback, positionSeconds: elapsed };
    return;
  }
  const next = chooseBrowserNext();
  if (next === null) {
    browserPlayback = { ...browserPlayback, status: "stopped", positionSeconds: duration };
    return;
  }
  browserStartedAt = performance.now();
  browserPlayback = {
    ...browserPlayback,
    currentIndex: next,
    currentTrack: browserPlayback.queue[next],
    positionSeconds: 0,
  };
}

async function command<T = PlaybackSnapshot>(name: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(name, args);
}

export async function getPlaybackSnapshot(): Promise<PlaybackSnapshot> {
  if (isTauriRuntime()) return command("playback_state");
  refreshBrowserClock();
  return cloneBrowserPlayback();
}

export async function rebindPlaybackCatalog(): Promise<PlaybackCatalogRebind> {
  if (isTauriRuntime()) return command<PlaybackCatalogRebind>("playback_rebind_catalog");
  refreshBrowserClock();
  return {
    playback: cloneBrowserPlayback(),
    catalogRevision: "0:0:",
  };
}

export async function playTrackQueue(tracks: Track[], startTrackId: string): Promise<PlaybackSnapshot> {
  browserPriorityEnd = null;
  if (isTauriRuntime()) {
    const startTrack = tracks.find((track) => track.id === startTrackId);
    if (!startTrack) throw new Error("The selected track is not part of this queue.");
    return command("playback_replace_queue", {
      trackReferences: tracks.map((track) => ({ id: track.id, trackKey: track.trackKey })),
      startTrackKey: startTrack.trackKey,
    });
  }
  const currentIndex = tracks.findIndex((track) => track.id === startTrackId);
  if (currentIndex < 0) throw new Error("The selected track is not part of this queue.");
  browserStartedAt = performance.now();
  browserPlayback = {
    ...browserPlayback,
    queue: [...tracks],
    currentIndex,
    currentTrack: tracks[currentIndex],
    status: "playing",
    positionSeconds: 0,
    error: null,
  };
  return cloneBrowserPlayback();
}

/** User queue edits preserve repeated songs and fail atomically when the queue is full. */
export async function enqueueTrackQueue(tracks: Track[], next: boolean): Promise<PlaybackSnapshot> {
  if (!tracks.length || tracks.length > 200) throw new Error("Select between 1 and 200 songs for the queue.");
  if (isTauriRuntime()) return command("playback_enqueue", { trackReferences: tracks.map(({ id, trackKey }) => ({ id, trackKey })), next });
  refreshBrowserClock();
  const keepFrom = Math.max(0, (browserPlayback.currentIndex ?? 0) - 20);
  const queue = browserPlayback.queue.slice(keepFrom);
  if (queue.length + tracks.length > 200) throw new Error("The queue holds at most 200 songs. Remove songs or select fewer before adding.");
  const currentIndex = browserPlayback.currentIndex === null ? 0 : browserPlayback.currentIndex - keepFrom;
  const at = next && queue.length ? currentIndex + 1 : queue.length;
  queue.splice(at, 0, ...tracks);
  browserPriorityEnd = next && browserPlayback.currentIndex !== null ? at + tracks.length - 1 + Math.max(0, (browserPriorityEnd ?? -1) - (browserPlayback.currentIndex ?? 0)) : browserPriorityEnd === null ? null : browserPriorityEnd - keepFrom;
  browserPlayback = { ...browserPlayback, queue, currentIndex, currentTrack: queue[currentIndex] };
  return cloneBrowserPlayback();
}

export async function appendTrackQueue(tracks: Track[]): Promise<PlaybackSnapshot> {
  if (tracks.length === 0 || tracks.length > 100) throw new Error("Queue refill batches must contain between 1 and 100 tracks.");
  if (isTauriRuntime()) {
    return command("playback_append_queue", {
      trackReferences: tracks.map((track) => ({ id: track.id, trackKey: track.trackKey })),
    });
  }
  const current = browserPlayback.currentIndex;
  if (current === null) throw new Error("Choose a track before extending its queue.");
  const keepFrom = Math.max(0, current - 20);
  const queue = browserPlayback.queue.slice(keepFrom);
  const currentIndex = current - keepFrom;
  const keys = new Set(queue.map((track) => track.trackKey));
  for (const track of tracks) {
    if (queue.length >= 200) break;
    if (!keys.has(track.trackKey)) {
      queue.push(track);
      keys.add(track.trackKey);
    }
  }
  if (browserPriorityEnd !== null) browserPriorityEnd -= keepFrom;
  browserPlayback = {
    ...browserPlayback,
    queue,
    currentIndex,
    currentTrack: queue[currentIndex],
  };
  return cloneBrowserPlayback();
}

export async function togglePlayback(): Promise<PlaybackSnapshot> {
  if (isTauriRuntime()) return command("playback_toggle");
  refreshBrowserClock();
  if (!browserPlayback.currentTrack) throw new Error("Choose a track before starting playback.");
  if (browserPlayback.status === "playing") {
    browserPlayback = { ...browserPlayback, status: "paused" };
  } else {
    const duration = browserPlayback.currentTrack.durationSeconds ?? 0;
    const resumeAt = browserPlayback.status === "stopped"
      && duration > 0
      && browserPlayback.positionSeconds >= duration - 0.25
      ? 0
      : browserPlayback.positionSeconds;
    browserStartedAt = performance.now() - resumeAt * 1000;
    browserPlayback = { ...browserPlayback, status: "playing", positionSeconds: resumeAt, error: null };
  }
  return cloneBrowserPlayback();
}

export async function nextTrack(): Promise<PlaybackSnapshot> {
  if (isTauriRuntime()) return command("playback_next");
  const next = chooseBrowserNext();
  if (next === null) throw new Error("There is no next track in the queue.");
  browserStartedAt = performance.now();
  browserPlayback = {
    ...browserPlayback,
    currentIndex: next,
    currentTrack: browserPlayback.queue[next],
    status: "playing",
    positionSeconds: 0,
    error: null,
  };
  return cloneBrowserPlayback();
}

export async function previousTrack(): Promise<PlaybackSnapshot> {
  if (isTauriRuntime()) return command("playback_previous");
  refreshBrowserClock();
  const current = browserPlayback.currentIndex;
  if (current === null) throw new Error("The queue has no previous track.");
  const previous = browserPlayback.positionSeconds > 3 ? current : Math.max(0, current - 1);
  browserStartedAt = performance.now();
  browserPlayback = {
    ...browserPlayback,
    currentIndex: previous,
    currentTrack: browserPlayback.queue[previous],
    status: "playing",
    positionSeconds: 0,
    error: null,
  };
  return cloneBrowserPlayback();
}

export async function seekPlayback(positionSeconds: number): Promise<PlaybackSnapshot> {
  if (isTauriRuntime()) return command("playback_seek", { positionSeconds });
  const duration = browserPlayback.currentTrack?.durationSeconds ?? 0;
  const position = Math.min(Math.max(positionSeconds, 0), duration);
  browserStartedAt = performance.now() - position * 1000;
  browserPlayback = { ...browserPlayback, positionSeconds: position };
  return cloneBrowserPlayback();
}

export async function changePlaybackVolume(volume: number): Promise<PlaybackSnapshot> {
  if (isTauriRuntime()) return command("playback_set_volume", { volume });
  browserPlayback = { ...browserPlayback, volume: Math.min(Math.max(volume, 0), 1) };
  return cloneBrowserPlayback();
}

export async function changeShuffle(enabled: boolean): Promise<PlaybackSnapshot> {
  if (isTauriRuntime()) return command("playback_set_shuffle", { enabled });
  browserPlayback = { ...browserPlayback, shuffle: enabled };
  return cloneBrowserPlayback();
}

export async function changeRepeatMode(repeatMode: RepeatMode): Promise<PlaybackSnapshot> {
  if (isTauriRuntime()) return command("playback_set_repeat_mode", { repeatMode });
  browserPlayback = { ...browserPlayback, repeatMode };
  return cloneBrowserPlayback();
}

export async function removeQueueItem(index: number): Promise<PlaybackSnapshot> {
  if (isTauriRuntime()) return command("playback_remove_queue_item", { index });
  if (index < 0 || index >= browserPlayback.queue.length) throw new Error("This queue item no longer exists.");
  browserPriorityEnd = null;
  const queue = browserPlayback.queue.filter((_, itemIndex) => itemIndex !== index);
  let currentIndex = browserPlayback.currentIndex;
  if (queue.length === 0) {
    browserPlayback = { ...emptyPlayback, volume: browserPlayback.volume, shuffle: browserPlayback.shuffle, repeatMode: browserPlayback.repeatMode };
    return cloneBrowserPlayback();
  }
  if (currentIndex !== null && index < currentIndex) currentIndex -= 1;
  if (currentIndex !== null && currentIndex >= queue.length) currentIndex = queue.length - 1;
  browserPlayback = {
    ...browserPlayback,
    queue,
    currentIndex,
    currentTrack: currentIndex === null ? null : queue[currentIndex],
  };
  return cloneBrowserPlayback();
}

export async function moveQueueItem(from: number, to: number): Promise<PlaybackSnapshot> {
  if (isTauriRuntime()) return command("playback_move_queue_item", { from, to });
  if (from < 0 || to < 0 || from >= browserPlayback.queue.length || to >= browserPlayback.queue.length) {
    throw new Error("The queue changed before this reorder completed.");
  }
  const current = browserPlayback.currentIndex;
  const queue = [...browserPlayback.queue];
  const [track] = queue.splice(from, 1);
  queue.splice(to, 0, track);
  const currentIndex = current === null ? null : current === from ? to : from < current && to >= current ? current - 1 : from > current && to <= current ? current + 1 : current;
  browserPriorityEnd = null;
  browserPlayback = { ...browserPlayback, queue, currentIndex };
  return cloneBrowserPlayback();
}

export async function clearPlaybackQueue(): Promise<PlaybackSnapshot> {
  browserPriorityEnd = null;
  if (isTauriRuntime()) return command("playback_clear_queue");
  browserPlayback = { ...emptyPlayback, volume: browserPlayback.volume, shuffle: browserPlayback.shuffle, repeatMode: browserPlayback.repeatMode };
  return cloneBrowserPlayback();
}

export function usePlayback() {
  const [state, setState] = useState<PlaybackSnapshot>(emptyPlayback);
  const [isWorking, setIsWorking] = useState(false);
  const [commandError, setCommandError] = useState<string | null>(null);
  const [dismissedError, setDismissedError] = useState<string | null>(null);
  const activeCommandCountRef = useRef(0);
  const commandSequenceRef = useRef(0);
  const refreshInFlightRef = useRef(false);
  const latestNativeSequenceRef = useRef(0);
  const acceptSnapshot = useCallback((next: PlaybackSnapshot) => {
    if (next.eventSequence !== undefined) {
      if (next.eventSequence < latestNativeSequenceRef.current) return;
      latestNativeSequenceRef.current = next.eventSequence;
    }
    setState(next);
  }, []);

  const refresh = useCallback(async () => {
    if (activeCommandCountRef.current > 0 || refreshInFlightRef.current) return;
    refreshInFlightRef.current = true;
    const commandSequence = commandSequenceRef.current;
    try {
      const next = await getPlaybackSnapshot();
      if (
        activeCommandCountRef.current === 0
        && commandSequenceRef.current === commandSequence
      ) acceptSnapshot(next);
    } catch (error) {
      if (
        activeCommandCountRef.current === 0
        && commandSequenceRef.current === commandSequence
      ) setCommandError(error instanceof Error ? error.message : String(error));
    } finally {
      refreshInFlightRef.current = false;
    }
  }, [acceptSnapshot]);

  useEffect(() => {
    let firstRefresh: number | undefined;
    const stop = subscribeNativeEvent<PlaybackSnapshot>("playback://state", acceptSnapshot, () => {
      firstRefresh = window.setTimeout(() => void refresh(), 0);
    });
    const timer = window.setInterval(() => void refresh(), isTauriRuntime() ? 15_000 : 2_000);
    return () => {
      window.clearTimeout(firstRefresh);
      stop();
      window.clearInterval(timer);
    };
  }, [acceptSnapshot, refresh]);

  const runCommand = useCallback(async <T,>(
    action: () => Promise<T>,
    snapshotFor: (result: T) => PlaybackSnapshot,
  ) => {
    const sequence = ++commandSequenceRef.current;
    activeCommandCountRef.current += 1;
    setIsWorking(true);
    setCommandError(null);
    setDismissedError(null);
    try {
      const next = await action();
      if (commandSequenceRef.current === sequence) acceptSnapshot(snapshotFor(next));
      return next;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (commandSequenceRef.current === sequence) setCommandError(message);
      return null;
    } finally {
      activeCommandCountRef.current = Math.max(0, activeCommandCountRef.current - 1);
      if (activeCommandCountRef.current === 0) setIsWorking(false);
    }
  }, [acceptSnapshot]);
  const run = useCallback(
    (action: () => Promise<PlaybackSnapshot>) => runCommand(action, (snapshot) => snapshot),
    [runCommand],
  );

  const visibleError = commandError ?? state.error;

  const refreshTrack = useCallback((updated: Track, includeEditableMetadata = false) => {
    const project = includeEditableMetadata
      ? applyEditableTrackTagProjection
      : applyTrackTagProjection;
    if (!isTauriRuntime()) {
      const queue = browserPlayback.queue.map((track) => track.trackKey === updated.trackKey
        ? project(track, updated)
        : track);
      browserPlayback = {
        ...browserPlayback,
        queue,
        currentTrack: browserPlayback.currentTrack?.trackKey === updated.trackKey
          ? project(browserPlayback.currentTrack, updated)
          : browserPlayback.currentTrack,
      };
    }
    setState((current) => ({
      ...current,
      queue: current.queue.map((track) => track.trackKey === updated.trackKey
        ? project(track, updated)
        : track),
      currentTrack: current.currentTrack?.trackKey === updated.trackKey
        ? project(current.currentTrack, updated)
        : current.currentTrack,
    }));
  }, []);

  const applySnapshot = useCallback((snapshot: PlaybackSnapshot) => {
    commandSequenceRef.current += 1;
    setCommandError(null);
    setDismissedError(null);
    acceptSnapshot(snapshot);
  }, [acceptSnapshot]);

  const append = useCallback(
    (tracks: Track[]) => run(() => appendTrackQueue(tracks)),
    [run],
  );
  const rebindCatalog = useCallback(
    () => runCommand(rebindPlaybackCatalog, (result) => result.playback),
    [runCommand],
  );

  return {
    state,
    isWorking,
    error: visibleError === dismissedError ? null : visibleError,
    dismissError: () => {
      setDismissedError(visibleError);
      setCommandError(null);
    },
    play: (tracks: Track[], startTrackId: string) => run(() => playTrackQueue(tracks, startTrackId)),
    append,
    enqueue: (tracks: Track[], next: boolean) => run(() => enqueueTrackQueue(tracks, next)),
    rebindCatalog,
    toggle: () => run(togglePlayback),
    next: () => run(nextTrack),
    previous: () => run(previousTrack),
    seek: (positionSeconds: number) => run(() => seekPlayback(positionSeconds)),
    setVolume: (volume: number) => run(() => changePlaybackVolume(volume)),
    setShuffle: (enabled: boolean) => run(() => changeShuffle(enabled)),
    setRepeatMode: (repeatMode: RepeatMode) => run(() => changeRepeatMode(repeatMode)),
    remove: (index: number) => run(() => removeQueueItem(index)),
    move: (from: number, to: number) => run(() => moveQueueItem(from, to)),
    clear: () => run(clearPlaybackQueue),
    refreshTrack,
    applySnapshot,
  };
}
