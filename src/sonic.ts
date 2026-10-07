import { invoke } from "@tauri-apps/api/core";
import { browserPreview, isTauriRuntime, type Track } from "./library";
export interface SonicRequest { seedKey: string; excludeKeys: string[]; limit: number; minimumRating: number | null; sameGenre: boolean; radio: boolean; }
export interface SonicResponse { analyzed: number; total: number; seedReady: boolean; tracks: Track[]; }
export async function sonicMatches(request: SonicRequest): Promise<SonicResponse> {
  if (isTauriRuntime()) return invoke("sonic_matches", { request });
  const seed = browserPreview.tracks.find(t => t.trackKey === request.seedKey);
  const excluded = new Set([...request.excludeKeys, request.seedKey]);
  return { analyzed: browserPreview.tracks.filter(t => t.sonicAnalyzed).length, total: browserPreview.summary.songs, seedReady: Boolean(seed?.sonicAnalyzed),
    tracks: browserPreview.tracks.filter(t => t.sonicAnalyzed && !excluded.has(t.trackKey) && t.loveState !== "banned" && (request.minimumRating === null || (t.rating ?? -1) >= request.minimumRating) && (!request.sameGenre || t.genre === seed?.genre)).slice(0, request.limit) };
}
export async function analyzeSonicSeed(trackKey: string): Promise<void> {
  if (isTauriRuntime()) await invoke("sonic_analyze_seed", { trackKey });
  else { const track = browserPreview.tracks.find(t => t.trackKey === trackKey); if (track) track.sonicAnalyzed = true; }
}
