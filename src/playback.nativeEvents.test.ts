import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen, type EventCallback } from "@tauri-apps/api/event";
import { usePlayback, type PlaybackSnapshot } from "./playback";

vi.mock("./library", async (original) => ({ ...await original<typeof import("./library")>(), isTauriRuntime: () => true }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); vi.useRealTimers(); });

function snapshot(eventSequence: number, status: PlaybackSnapshot["status"]): PlaybackSnapshot {
  return {
    eventSequence, status, queue: [], currentIndex: null, currentTrack: null,
    positionSeconds: 0, volume: 0.7, shuffle: false, repeatMode: "off", error: null,
    outputDeviceLabel: null, usingDeviceFallback: false, replayGainMode: "off",
    replayGainDb: null, replayGainSource: null, clippingPrevented: false,
    audioUnderrunCount: 0, realtimeSchedulingDenied: false,
  };
}

it("applies native transitions immediately and rejects an older in-flight heartbeat", async () => {
  vi.useFakeTimers();
  const release = vi.fn();
  vi.mocked(listen).mockResolvedValue(release);
  vi.mocked(invoke).mockResolvedValueOnce(snapshot(1, "playing"));
  const { result, unmount } = renderHook(usePlayback);
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  const receive = vi.mocked(listen).mock.calls[0][1] as EventCallback<PlaybackSnapshot>;
  const emit = (value: PlaybackSnapshot) => receive({ event: "playback://state", id: 1, payload: value });
  await act(async () => { emit(snapshot(2, "paused")); });
  expect(result.current.state.status).toBe("paused");
  await act(async () => { await vi.advanceTimersByTimeAsync(14_999); });
  expect(invoke).toHaveBeenCalledTimes(1);
  let finish!: (value: PlaybackSnapshot) => void;
  vi.mocked(invoke).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  await act(async () => { await vi.advanceTimersByTimeAsync(1); });
  expect(invoke).toHaveBeenCalledTimes(2);
  await act(async () => { emit(snapshot(4, "playing")); finish(snapshot(3, "paused")); });
  expect(result.current.state.eventSequence).toBe(4);
  expect(result.current.state.status).toBe("playing");
  unmount();
  expect(release).toHaveBeenCalledOnce();
  expect(vi.getTimerCount()).toBe(0);
});
