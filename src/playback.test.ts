import { describe, expect, it } from "vitest";
import { browserPreview } from "./library";
import {
  changeRepeatMode,
  changeShuffle,
  clearPlaybackQueue,
  moveQueueItem,
  playTrackQueue,
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
});
