import { describe, expect, it } from "vitest";
import { browserPreview } from "./library";
import {
  appendTrackQueue,
  changeRepeatMode,
  changeShuffle,
  clearPlaybackQueue,
  getPlaybackSnapshot,
  moveQueueItem,
  playTrackQueue,
  rebindPlaybackCatalog,
  seekPlayback,
  togglePlayback,
} from "./playback";

describe("browser playback adapter", () => {
  it("exercises the same queue and transport contract as the native boundary", async () => {
    const tracks = browserPreview.tracks.slice(0, 3);
    let state = await playTrackQueue(tracks, tracks[1].id);
    expect(state.status).toBe("playing");
    expect(state.currentTrack?.id).toBe(tracks[1].id);

    state = await togglePlayback();
    expect(state.status).toBe("paused");
    state = await seekPlayback(90);
    expect(state.positionSeconds).toBe(90);
    state = await changeShuffle(true);
    expect(state.shuffle).toBe(true);
    state = await changeRepeatMode("all");
    expect(state.repeatMode).toBe("all");

    state = await moveQueueItem(1, 0);
    expect(state.currentIndex).toBe(0);
    expect(state.currentTrack?.id).toBe(tracks[1].id);

    state = await clearPlaybackQueue();
    expect(state.queue).toEqual([]);
    expect(state.status).toBe("stopped");
  });

  it("restarts a completed track instead of resuming at its end", async () => {
    const track = browserPreview.tracks[0];
    await playTrackQueue([track], track.id);
    await changeRepeatMode("off");
    await seekPlayback(track.durationSeconds ?? 0);
    await new Promise((resolve) => setTimeout(resolve, 5));
    let state = await getPlaybackSnapshot();
    expect(state.status).toBe("stopped");
    state = await togglePlayback();
    expect(state.status).toBe("playing");
    expect(state.positionSeconds).toBe(0);
  });

  it("refills a bounded queue without replacing the current track", async () => {
    const source = browserPreview.tracks[0];
    const initial = Array.from({ length: 200 }, (_, index) => ({
      ...source,
      id: `initial-${index}`,
      trackKey: `preview:initial-${index}`,
      title: `Initial ${index}`,
    }));
    await playTrackQueue(initial, initial[181].id);
    const additions = Array.from({ length: 100 }, (_, index) => ({
      ...source,
      id: `addition-${index}`,
      trackKey: `preview:addition-${index}`,
      title: `Addition ${index}`,
    }));
    const state = await appendTrackQueue(additions);
    expect(state.currentTrack?.id).toBe("initial-181");
    expect(state.currentIndex).toBe(20);
    expect(state.queue).toHaveLength(139);
    expect(state.queue[state.queue.length - 1]?.id).toBe("addition-99");
  });

  it("keeps browser playback intact when the catalog revision is rebound", async () => {
    const tracks = browserPreview.tracks.slice(0, 3);
    const playing = await playTrackQueue(tracks, tracks[1].id);
    const rebound = await rebindPlaybackCatalog();

    expect(rebound.catalogRevision).toBe("0:0:");
    expect(rebound.playback.status).toBe(playing.status);
    expect(rebound.playback.currentTrack?.trackKey).toBe(tracks[1].trackKey);
    expect(rebound.playback.queue.map((track) => track.trackKey)).toEqual(
      tracks.map((track) => track.trackKey),
    );
  });
});
