import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, loadLibrarySnapshot, type Track } from "./library";

export type HistoryOutcomeFilter = "all" | "played" | "completed" | "skipped" | "interrupted";
export type HistoryOutcome = "active" | "completed" | "skipped" | "interrupted";

export interface HistoryPageRequest {
  pageSize: number;
  cursor?: string;
  search?: string;
  deviceId?: string;
  outcome?: HistoryOutcomeFilter;
  startedAfterMs?: number;
  startedBeforeMs?: number;
}

export interface HistoryItem {
  sessionId: string;
  trackKey: string;
  title: string;
  artist: string;
  album: string;
  genre: string | null;
  durationSeconds: number | null;
  deviceId: string;
  deviceName: string;
  startedAtMs: number;
  endedAtMs: number | null;
  listenedSeconds: number;
  registeredPlay: boolean;
  registeredAtMs: number | null;
  outcome: HistoryOutcome;
  track: Track | null;
}

export interface HistorySummary {
  sessions: number;
  plays: number;
  skips: number;
  uniqueTracks: number;
  listenedSeconds: number;
}

export interface HistoryDevice {
  deviceId: string;
  deviceName: string;
  sessions: number;
  lastListenedAtMs: number | null;
  isThisDevice: boolean;
}

export interface HistoryTopTrack {
  trackKey: string;
  title: string;
  artist: string;
  album: string;
  plays: number;
  listenedSeconds: number;
  lastPlayedAtMs: number;
  track: Track | null;
}

export interface HistoryPage {
  items: HistoryItem[];
  summary: HistorySummary;
  topTracks: HistoryTopTrack[];
  devices: HistoryDevice[];
  nextCursor: string | null;
  playThresholdSeconds: number;
  syncState: "synced" | "unavailable";
  syncMessage: string;
}

export interface TrackHistoryInsight {
  sessions: number;
  plays: number;
  skips: number;
  listenedSeconds: number;
  lastListenedAtMs: number | null;
}

let previewThresholdSeconds = 30;

function previewItems(tracks: Track[]): HistoryItem[] {
  const now = Date.now();
  const outcomes: HistoryOutcome[] = [
    "completed", "completed", "skipped", "completed", "interrupted", "completed", "skipped",
    "completed", "completed", "completed", "skipped", "completed", "completed", "interrupted",
  ];
  return outcomes.map((outcome, index) => {
    const track = tracks[index % tracks.length];
    const duration = track.durationSeconds ?? 240;
    const registeredPlay = outcome === "completed" || (index === 4 && duration >= previewThresholdSeconds);
    const listenedSeconds = outcome === "completed"
      ? duration
      : registeredPlay
        ? previewThresholdSeconds + 18
        : Math.min(previewThresholdSeconds - 1, 8 + index * 2);
    const startedAtMs = now - index * 3_780_000 - (index > 5 ? 86_400_000 : 0);
    return {
      sessionId: `preview-session-${index}`,
      trackKey: track.trackKey,
      title: track.title,
      artist: track.artist,
      album: track.album,
      genre: track.genre,
      durationSeconds: track.durationSeconds,
      deviceId: index % 4 === 0 ? "device-keiya-preview" : "device-jorncomputer-preview",
      deviceName: index % 4 === 0 ? "Keiya" : "JornComputer",
      startedAtMs,
      endedAtMs: startedAtMs + Math.round(listenedSeconds * 1_000),
      listenedSeconds,
      registeredPlay,
      registeredAtMs: registeredPlay ? startedAtMs + previewThresholdSeconds * 1_000 : null,
      outcome,
      track,
    };
  });
}

function matchesPreview(item: HistoryItem, request: HistoryPageRequest): boolean {
  const search = request.search?.trim().toLocaleLowerCase();
  if (search && !`${item.title} ${item.artist} ${item.album} ${item.genre ?? ""}`.toLocaleLowerCase().includes(search)) return false;
  if (request.deviceId && item.deviceId !== request.deviceId) return false;
  const outcome = request.outcome ?? "all";
  if (outcome === "played" && !item.registeredPlay) return false;
  if (outcome !== "all" && outcome !== "played" && item.outcome !== outcome) return false;
  if (request.startedAfterMs !== undefined && item.startedAtMs < request.startedAfterMs) return false;
  if (request.startedBeforeMs !== undefined && item.startedAtMs > request.startedBeforeMs) return false;
  return true;
}

function previewTopTracks(items: HistoryItem[]): HistoryTopTrack[] {
  const aggregate = new Map<string, HistoryTopTrack>();
  for (const item of items.filter((candidate) => candidate.registeredPlay)) {
    const current = aggregate.get(item.trackKey);
    if (current) {
      current.plays += 1;
      current.listenedSeconds += item.listenedSeconds;
      current.lastPlayedAtMs = Math.max(current.lastPlayedAtMs, item.startedAtMs);
    } else {
      aggregate.set(item.trackKey, {
        trackKey: item.trackKey,
        title: item.title,
        artist: item.artist,
        album: item.album,
        plays: 1,
        listenedSeconds: item.listenedSeconds,
        lastPlayedAtMs: item.startedAtMs,
        track: item.track,
      });
    }
  }
  return [...aggregate.values()]
    .sort((left, right) => right.plays - left.plays || right.listenedSeconds - left.listenedSeconds)
    .slice(0, 8);
}

async function previewHistoryPage(request: HistoryPageRequest): Promise<HistoryPage> {
  const snapshot = await loadLibrarySnapshot();
  const allItems = previewItems(snapshot.tracks);
  const items = allItems.filter((item) => matchesPreview(item, request)).slice(0, request.pageSize);
  const plays = allItems.filter((item) => item.registeredPlay);
  return {
    items,
    summary: {
      sessions: allItems.length,
      plays: plays.length,
      skips: allItems.filter((item) => item.outcome === "skipped").length,
      uniqueTracks: new Set(plays.map((item) => item.trackKey)).size,
      listenedSeconds: allItems.reduce((total, item) => total + item.listenedSeconds, 0),
    },
    topTracks: previewTopTracks(allItems),
    devices: [
      { deviceId: "device-jorncomputer-preview", deviceName: "JornComputer", sessions: 10, lastListenedAtMs: allItems[0]?.startedAtMs ?? null, isThisDevice: true },
      { deviceId: "device-keiya-preview", deviceName: "Keiya", sessions: 4, lastListenedAtMs: allItems[4]?.startedAtMs ?? null, isThisDevice: false },
    ],
    nextCursor: null,
    playThresholdSeconds: previewThresholdSeconds,
    syncState: "synced",
    syncMessage: "Browser preview: both device histories are available.",
  };
}

export async function loadHistoryPage(request: HistoryPageRequest): Promise<HistoryPage> {
  if (!isTauriRuntime()) return previewHistoryPage(request);
  return invoke<HistoryPage>("listening_history_page", { request });
}

export async function loadTrackHistoryInsight(trackKey: string): Promise<TrackHistoryInsight> {
  if (!isTauriRuntime()) {
    const snapshot = await loadLibrarySnapshot();
    const items = previewItems(snapshot.tracks).filter((item) => item.trackKey === trackKey);
    return {
      sessions: items.length,
      plays: items.filter((item) => item.registeredPlay).length,
      skips: items.filter((item) => item.outcome === "skipped").length,
      listenedSeconds: items.reduce((total, item) => total + item.listenedSeconds, 0),
      lastListenedAtMs: items.reduce<number | null>((latest, item) => latest === null ? item.startedAtMs : Math.max(latest, item.startedAtMs), null),
    };
  }
  return invoke<TrackHistoryInsight>("track_history_insight", { trackKey });
}

export async function saveHistoryPlayThreshold(playThresholdSeconds: number): Promise<number> {
  if (!Number.isInteger(playThresholdSeconds) || playThresholdSeconds < 1 || playThresholdSeconds > 3_600) {
    throw new Error("Played threshold must be between 1 and 3600 seconds.");
  }
  if (!isTauriRuntime()) {
    previewThresholdSeconds = playThresholdSeconds;
    return previewThresholdSeconds;
  }
  return invoke<number>("set_history_play_threshold", { playThresholdSeconds });
}

export function resetHistoryPreview(): void {
  previewThresholdSeconds = 30;
}
