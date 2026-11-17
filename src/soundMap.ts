import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, type Track } from "./library";
import { WAVEFORM_PEAK_COUNT } from "./waveform";

/**
 * The coloured seek bar's data, from Music Library's per-second timeline:
 * brightness and stereo width in step with the waveform's peaks, the
 * seconds where the sound changes, and where the main part starts.
 */
export interface SoundMap {
  trackKey: string;
  seconds: number;
  /** 0 dull to 1 bright, or null where the track is silent. */
  brightness: (number | null)[];
  /** 0 narrow to 1 wide, or null where silent or mono. */
  width: (number | null)[];
  sections: number[];
  /** After a long quiet intro, the second the main part starts. */
  mainStart: number | null;
}

export type SeekBarColour = "brightness" | "width" | "plain";

export const SEEK_BAR_COLOURS: readonly SeekBarColour[] = ["brightness", "width", "plain"];

export const seekBarColourNames: Record<SeekBarColour, string> = {
  brightness: "Brightness",
  width: "Stereo width",
  plain: "Plain",
};

const STORAGE_KEY = "aurora.seekBarColour";

export function loadSeekBarColour(): SeekBarColour {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    return SEEK_BAR_COLOURS.includes(stored as SeekBarColour) ? stored as SeekBarColour : "brightness";
  } catch {
    return "brightness";
  }
}

export function saveSeekBarColour(colour: SeekBarColour): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, colour);
  } catch {
    // The choice lasts for this session only.
  }
}

export function nextSeekBarColour(colour: SeekBarColour): SeekBarColour {
  return SEEK_BAR_COLOURS[(SEEK_BAR_COLOURS.indexOf(colour) + 1) % SEEK_BAR_COLOURS.length];
}

type Rgb = readonly [number, number, number];

/** Dull to bright: deep blue, violet, gold. */
const BRIGHTNESS_STOPS: readonly Rgb[] = [[59, 76, 192], [192, 86, 214], [255, 209, 102]];
/** Narrow to wide: orange, violet, cyan. */
const WIDTH_STOPS: readonly Rgb[] = [[255, 138, 61], [168, 85, 247], [34, 211, 238]];
const SILENT: Rgb = [91, 101, 115];

function blend(stops: readonly Rgb[], value: number): Rgb {
  const scaled = Math.min(Math.max(value, 0), 1) * (stops.length - 1);
  const index = Math.min(Math.floor(scaled), stops.length - 2);
  const amount = scaled - index;
  const from = stops[index];
  const to = stops[index + 1];
  const mix = (channel: 0 | 1 | 2) => Math.round(from[channel] + (to[channel] - from[channel]) * amount);
  return [mix(0), mix(1), mix(2)];
}

/** The CSS colour of one bin of the seek bar. */
export function seekBarColour(colour: Exclude<SeekBarColour, "plain">, value: number | null): string {
  const [red, green, blue] = value === null ? SILENT : blend(colour === "brightness" ? BRIGHTNESS_STOPS : WIDTH_STOPS, value);
  return `rgb(${red}, ${green}, ${blue})`;
}

function previewMap(track: Pick<Track, "trackKey" | "durationSeconds">): SoundMap {
  const seconds = Math.max(track.durationSeconds ?? 240, 1);
  const lane = (phase: number) => Array.from({ length: WAVEFORM_PEAK_COUNT }, (_, index) => {
    const along = index / WAVEFORM_PEAK_COUNT;
    return along < 0.12 ? 0.15 : 0.5 + Math.sin(along * 9 + phase) * 0.35;
  });
  return {
    trackKey: track.trackKey,
    seconds,
    brightness: lane(0),
    width: lane(1.8),
    sections: [0.12, 0.34, 0.58, 0.81].map((share) => Math.round(seconds * share)),
    mainStart: seconds >= 120 ? Math.round(seconds * 0.12) : null,
  };
}

export async function loadTrackSoundMap(track: Pick<Track, "id" | "trackKey" | "durationSeconds">): Promise<SoundMap | null> {
  if (!isTauriRuntime()) return previewMap(track);
  const map = await invoke<SoundMap | null>("track_sound_map", { trackId: track.id, trackKey: track.trackKey });
  if (map && (map.brightness.length !== WAVEFORM_PEAK_COUNT || map.width.length !== WAVEFORM_PEAK_COUNT)) {
    throw new Error("Aurora received an invalid sound map.");
  }
  return map;
}
