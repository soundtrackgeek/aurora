import { describe, expect, it } from "vitest";
import type { Track } from "./library";
import { WAVEFORM_PEAK_COUNT, loadTrackWaveform, validateTrackWaveform } from "./waveform";

const track = {
  id: "1",
  trackKey: "d:/music/m83/midnight city.mp3",
} as Track;

describe("waveform adapter", () => {
  it("provides a stable, bounded waveform in browser preview", async () => {
    const first = await loadTrackWaveform(track);
    const second = await loadTrackWaveform(track);
    expect(first.peaks).toHaveLength(WAVEFORM_PEAK_COUNT);
    expect(first.peaks).toEqual(second.peaks);
    expect(first.peaks.every((peak) => peak >= 0 && peak <= 1)).toBe(true);
  });

  it("rejects malformed native waveform data", () => {
    expect(() => validateTrackWaveform({
      trackKey: track.trackKey,
      peaks: [1.5],
      sampleRate: 44_100,
      channels: 2,
      source: "decoded",
    })).toThrow("invalid waveform");
  });
});
