import { invoke } from "@tauri-apps/api/core";
import { browserPreview, isTauriRuntime } from "./library";
import { searchJourneyTracks } from "./sonicJourney";
import type { SonicTrack } from "./hiddenFavorites";
export type { SonicTrack };
export { searchJourneyTracks as searchAtlasTracks };
export interface AtlasRequest { action: string; seedKey: string; revision: string | null; regionId: string | null; trackKeys: string[] | null; name: string | null }
export interface AtlasCounts { owned: number; rated: number; loved: number; absentFromHistory: number; unavailable: number }
export interface AtlasPoint { track: SonicTrack; x: number; y: number; available: boolean; inHistory: boolean }
export interface AtlasRegion { id: string; representativeKey: string; label: string; points: AtlasPoint[]; counts: AtlasCounts }
export interface SonicAtlas { version: string; profile: string; revision: string; generatedAt: string; seed: SonicTrack; regions: AtlasRegion[]; counts: AtlasCounts; historyPlays: number; historySince: number | null; message: string | null }
export function atlasCounts(points: AtlasPoint[]): AtlasCounts {
  const available = points.filter(p => p.available);
  return { owned: available.length, rated: available.filter(p => (p.track.rating ?? 0) > 0).length, loved: available.filter(p => p.track.loved).length,
    absentFromHistory: available.filter(p => !p.inHistory).length, unavailable: points.length - available.length };
}
export function demoAtlas(tracks: SonicTrack[], seedKey: string): SonicAtlas {
  const seed = tracks.find(t => t.trackKey === seedKey) ?? tracks[0];
  const regions = Array.from({ length: Math.min(3, tracks.length) }, (_, i) => {
    const members = tracks.filter((_, n) => n % 3 === i);
    const points = members.map((track, n) => ({ track, x: Math.min(.95, .12 + i * .3 + (n % 3) * .035), y: Math.min(.92, .2 + i * .24 + Math.floor(n / 3) * .04), available: true, inHistory: n % 3 === 0 }));
    return { id: members[0].trackKey, label: `Near ${members[0].title}`, representativeKey: members[0].trackKey, points, counts: atlasCounts(points) };
  });
  return { version: "sonic-atlas-v1", profile: "Browser demonstration · illustrative positions and regions", revision: `preview-${seedKey}`, generatedAt: new Date().toISOString(), seed,
    regions, counts: atlasCounts(regions.flatMap(r => r.points)), historyPlays: 12, historySince: null, message: null };
}
const cache = new Map<string, SonicAtlas>();
export async function operateSonicAtlas(request: AtlasRequest): Promise<SonicAtlas> {
  if (isTauriRuntime()) return invoke("sonic_atlas", { request });
  let atlas = cache.get(request.seedKey);
  if (!atlas || request.action === "rebuild") {
    const tracks = browserPreview.tracks.filter(t => t.loveState !== "banned").map(t => ({ trackId: Number(t.id.replace("preview-", "")), trackKey: t.trackKey, title: t.title, artist: t.displayArtist ?? t.artist, albumArtist: t.artist, album: t.album, albumId: t.albumId ?? "", filePath: "Preview", filename: `${t.id}.mp3`, genre: t.genre, rating: t.rating === null ? null : t.rating * 20, seconds: t.durationSeconds ?? 180, loved: t.loved, distance: null }));
    atlas = demoAtlas(tracks, request.seedKey); cache.set(request.seedKey, atlas);
  }
  return { ...atlas, message: request.action === "save" ? `Browser demonstration: saved ${request.name}.` : null };
}
