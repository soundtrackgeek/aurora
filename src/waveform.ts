import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, type Track } from "./library";

export const WAVEFORM_PEAK_COUNT = 320;

export interface TrackWaveform {
  trackKey: string;
  peaks: number[];
  sampleRate: number | null;
  channels: number | null;
  source: "decoded" | "cache" | "browserPreview";
}

function previewPeaks(trackKey: string): number[] {
  let seed = [...trackKey].reduce(
    (value, character) => (Math.imul(value, 31) + (character.codePointAt(0) ?? 0)) >>> 0,
    2_166_136_261,
  );
  return Array.from({ length: WAVEFORM_PEAK_COUNT }, (_, index) => {
    seed = (Math.imul(seed, 1_664_525) + 1_013_904_223) >>> 0;
    const noise = seed / 0xffff_ffff;
    const envelope = 0.46 + Math.sin(index * 0.082) * 0.13 + Math.sin(index * 0.021 + 1.7) * 0.12;
    return Math.min(1, Math.max(0.06, (0.22 + noise * 0.78) * envelope));
  });
}

export function validateTrackWaveform(value: TrackWaveform): TrackWaveform {
  if (
    value.peaks.length !== WAVEFORM_PEAK_COUNT
    || value.peaks.some((peak) => !Number.isFinite(peak) || peak < 0 || peak > 1)
  ) {
    throw new Error("Aurora received an invalid waveform from the audio decoder.");
  }
  return value;
}

export async function loadTrackWaveform(track: Pick<Track, "id" | "trackKey">): Promise<TrackWaveform> {
  if (!isTauriRuntime()) {
    return {
      trackKey: track.trackKey,
      peaks: previewPeaks(track.trackKey),
      sampleRate: 44_100,
      channels: 2,
      source: "browserPreview",
    };
  }
  return validateTrackWaveform(await invoke<TrackWaveform>("track_waveform", {
    trackId: track.id,
    trackKey: track.trackKey,
  }));
}
