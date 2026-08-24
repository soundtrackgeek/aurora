import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import type { Track } from "../library";
import type { PlaybackSnapshot } from "../playback";
import { PlayerBar } from "./PlayerBar";

class PreviewResizeObserver {
  observe() {}
  disconnect() {}
  unobserve() {}
}

beforeAll(() => {
  vi.stubGlobal("ResizeObserver", PreviewResizeObserver);
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
});

afterEach(cleanup);

const track: Track = {
  id: "1",
  trackKey: "preview:1",
  albumId: null,
  title: "Midnight City",
  artist: "M83",
  album: "Hurry Up, We're Dreaming",
  releaseYear: 2011,
  rating: 4,
  loved: false,
  loveState: "neutral",
  tagSyncState: null,
  canUndoTagEdit: false,
  durationSeconds: 240,
  genre: "Electronic",
  playCount: 10,
};

function snapshot(positionSeconds: number, currentTrack = track): PlaybackSnapshot {
  return {
    queue: [currentTrack],
    currentIndex: 0,
    currentTrack,
    status: "playing",
    positionSeconds,
    volume: 0.7,
    shuffle: false,
    repeatMode: "off",
    error: null,
    outputDeviceLabel: "Speakers (Realtek Audio)",
    usingDeviceFallback: false,
    replayGainMode: "track",
    replayGainDb: -6.4,
    replayGainSource: "track",
    clippingPrevented: false,
    audioUnderrunCount: 0,
    realtimeSchedulingDenied: false,
  };
}

function props(positionSeconds = 60) {
  return {
    playback: snapshot(positionSeconds),
    isWorking: false,
    tagBusy: false,
    error: null,
    queueOpen: false,
    onDismissError: vi.fn(),
    onToggle: vi.fn(),
    onPrevious: vi.fn(),
    onNext: vi.fn(),
    onSeek: vi.fn(),
    onVolume: vi.fn(),
    onShuffle: vi.fn(),
    onRepeat: vi.fn(),
    onRatingChange: vi.fn(),
    onLoveChange: vi.fn(),
    onOpenAudioSettings: vi.fn(),
    onToggleQueue: vi.fn(),
  };
}

describe("PlayerBar", () => {
  it("shows the track artist instead of the album artist", () => {
    const soundtrackTrack: Track = {
      ...track,
      title: "Miracles",
      artist: "Alexandre Desplat",
      displayArtist: "Coldplay",
      album: "Unbroken",
    };
    render(<PlayerBar {...props()} playback={snapshot(60, soundtrackTrack)} />);

    expect(screen.getByText("Coldplay · Unbroken")).toBeInTheDocument();
    expect(screen.queryByText("Alexandre Desplat · Unbroken")).not.toBeInTheDocument();
  });

  it("toggles total time to a live remaining-time readout", () => {
    const initial = props();
    const { rerender } = render(<PlayerBar {...initial} />);
    const endTime = screen.getByRole("button", { name: "Show remaining track time" });
    expect(endTime).toHaveTextContent("4:00");
    fireEvent.click(endTime);
    expect(screen.getByRole("button", { name: "Show total track length" })).toHaveTextContent("−3:00");

    rerender(<PlayerBar {...props(61)} />);
    expect(screen.getByRole("button", { name: "Show total track length" })).toHaveTextContent("−2:59");
  });

  it("keeps the playback position moving between native snapshots", () => {
    vi.useFakeTimers();
    let now = 0;
    const nowSpy = vi.spyOn(performance, "now").mockImplementation(() => now);
    try {
      render(<PlayerBar {...props()} />);
      const timeline = screen.getByRole("slider", { name: "Playback position" });
      expect(timeline).toHaveValue("60");

      now = 1_250;
      act(() => vi.advanceTimersByTime(1_250));
      expect(timeline).toHaveValue("61.25");
    } finally {
      nowSpy.mockRestore();
      vi.useRealTimers();
    }
  });

  it("routes rating clears and Love through instant player callbacks", () => {
    const playerProps = props();
    const player = render(<PlayerBar {...playerProps} />);
    fireEvent.click(player.getByRole("button", { name: "Clear rating for Midnight City" }));
    fireEvent.click(player.getByRole("button", { name: "Love Midnight City" }));
    expect(playerProps.onRatingChange).toHaveBeenCalledWith(track, null);
    expect(playerProps.onLoveChange).toHaveBeenCalledWith(track, "loved");
  });

  it("shows the active output and applied gain and opens audio settings", () => {
    const playerProps = props();
    render(<PlayerBar {...playerProps} />);
    const audio = screen.getByRole("button", { name: /Open audio settings.*Speakers.*−?6\.4 dB/i });
    fireEvent.click(audio);
    expect(playerProps.onOpenAudioSettings).toHaveBeenCalledOnce();
  });

  it("surfaces native audio health diagnostics in the output readout", () => {
    const playback = {
      ...snapshot(60),
      audioUnderrunCount: 2,
      realtimeSchedulingDenied: true,
    };
    render(<PlayerBar {...props()} playback={playback} />);

    expect(screen.getByRole("button", { name: /2 audio underruns/i })).toBeInTheDocument();
  });

  it("releases the seek draft after the committed seek finishes", async () => {
    let finishSeek: (() => void) | undefined;
    const pendingSeek = new Promise<void>((resolve) => { finishSeek = resolve; });
    const playerProps = { ...props(), onSeek: vi.fn(() => pendingSeek) };
    const player = render(<PlayerBar {...playerProps} />);
    const timeline = player.getByRole("slider", { name: "Playback position" });

    fireEvent.change(timeline, { target: { value: "180" } });
    fireEvent.pointerUp(timeline, { target: { value: "180" } });
    expect(playerProps.onSeek).toHaveBeenCalledWith(180);
    expect(timeline).toHaveValue("180");

    player.rerender(<PlayerBar {...playerProps} playback={snapshot(181)} />);
    expect(timeline).toHaveValue("180");

    await act(async () => { finishSeek?.(); });
    expect(timeline).toHaveValue("181");
  });

  it("keeps the newest seek draft when rapid seeks finish out of order", async () => {
    let finishFirst: (() => void) | undefined;
    let finishSecond: (() => void) | undefined;
    const firstSeek = new Promise<void>((resolve) => { finishFirst = resolve; });
    const secondSeek = new Promise<void>((resolve) => { finishSecond = resolve; });
    const playerProps = {
      ...props(),
      onSeek: vi.fn()
        .mockReturnValueOnce(firstSeek)
        .mockReturnValueOnce(secondSeek),
    };
    const player = render(<PlayerBar {...playerProps} />);
    const timeline = player.getByRole("slider", { name: "Playback position" });

    fireEvent.pointerUp(timeline, { target: { value: "180" } });
    fireEvent.pointerUp(timeline, { target: { value: "30" } });
    expect(playerProps.onSeek).toHaveBeenNthCalledWith(1, 180);
    expect(playerProps.onSeek).toHaveBeenNthCalledWith(2, 30);
    expect(timeline).toHaveValue("30");

    await act(async () => { finishFirst?.(); });
    expect(timeline).toHaveValue("30");

    player.rerender(<PlayerBar {...playerProps} playback={snapshot(31)} />);
    await act(async () => { finishSecond?.(); });
    expect(timeline).toHaveValue("31");
  });
});
