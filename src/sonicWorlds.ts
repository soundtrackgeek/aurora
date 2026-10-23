import { invoke } from "@tauri-apps/api/core";
import { browserPreview, displayTrackArtist, isTauriRuntime, type Track } from "./library";

export type WorldBoundary = "decade" | "albumArtist" | "genre";
export type WorldDirection = "stay" | "either" | "cross";
export interface WorldRequest { seedKey: string; boundary: WorldBoundary; direction: WorldDirection; excludeKeys: string[]; }
export interface WorldResponse {
  version: string; seedReady: boolean; seedValue: string | null; maxDistance: number; searchLimited: boolean;
  matches: { track: Track; value: string | null; distance: number }[];
}

function value(track: Track, boundary: WorldBoundary): string | null {
  if (boundary === "decade") return track.originalYear && track.originalYear >= 1000 && track.originalYear <= 9999 ? `${Math.floor(track.originalYear / 10) * 10}s` : null;
  const text = (boundary === "albumArtist" ? track.artist : track.genre)?.trim().replace(/\s+/g, " ");
  return text && !["unknown", "unknown artist", "unknown genre", "various artists"].includes(text.toLowerCase()) ? text : null;
}

export async function sonicWorlds(request: WorldRequest): Promise<WorldResponse> {
  if (isTauriRuntime()) return invoke("sonic_worlds", { request });
  const seed = browserPreview.tracks.find(t => t.trackKey === request.seedKey);
  const seedValue = seed ? value(seed, request.boundary) : null;
  const seedReady = Boolean(seed?.sonicAnalyzed && seed.loveState !== "banned");
  if (seedReady && request.direction !== "either" && !seedValue) throw new Error("The seed has no reliable catalog value for this boundary. Choose another boundary or Either.");
  const albums = new Set([seed?.albumId]);
  const credit = (t: Track) => `${t.title.trim().replace(/\s+/g, " ").toLowerCase()}\n${displayTrackArtist(t).trim().replace(/\s+/g, " ").toLowerCase()}`;
  const credits = new Set(seed ? [credit(seed)] : []);
  const matches = seedReady ? browserPreview.tracks.filter(t => {
    if (!t.sonicAnalyzed || t.loveState === "banned" || t.trackKey === request.seedKey || request.excludeKeys.includes(t.trackKey)) return false;
    const candidate = value(t, request.boundary);
    return request.direction === "either" || Boolean(seedValue && candidate && ((seedValue.toLowerCase() === candidate.toLowerCase()) === (request.direction === "stay")));
  }).filter(t => {
    if (t.albumId && albums.has(t.albumId) || credits.has(credit(t))) return false;
    if (t.albumId) albums.add(t.albumId);
    credits.add(credit(t));
    return true;
  }).slice(0, 3).map(track => ({ track, value: value(track, request.boundary), distance: 0.2 })) : [];
  return { version: "Browser demonstration", seedReady, seedValue, maxDistance: 0.25, searchLimited: true, matches };
}
