import { invoke } from "@tauri-apps/api/core";
import { browserPreview, isTauriRuntime, type Track } from "./library";
export interface SonicRequest { seedKey: string; seedAlbumId?: string | null; minimumCoverage?: number; excludeKeys: string[]; limit: number; minimumRating: number | null; sameGenre: boolean; radio: boolean; }
export interface SonicAlbum { albumId: string; title: string; albumArtist: string; genre: string | null; totalTracks: number; analyzedTracks: number; distance: number | null; }
export interface SonicAlbumMatches { seed: SonicAlbum | null; seedReady: boolean; analyzedAlbums: number; albums: SonicAlbum[]; }
export async function sonicAlbumMatches(albumId: string, minimumCoverage: number): Promise<SonicAlbumMatches> {
  if (isTauriRuntime()) return invoke("sonic_album_matches", { request: { albumId, minimumCoverage, limit: 20 } });
  const groups = new Map<string, Track[]>();
  for (const track of browserPreview.tracks) { if (track.albumId) groups.set(track.albumId, [...(groups.get(track.albumId) ?? []), track]); }
  const albums = [...groups].map(([id, tracks]): SonicAlbum => ({ albumId: id, title: tracks[0].album, albumArtist: tracks[0].artist, genre: tracks[0].genre,
    totalTracks: tracks.length, analyzedTracks: tracks.filter(t => t.sonicAnalyzed && t.loveState !== "banned").length, distance: 0.2 }));
  const ready = (a: SonicAlbum) => a.analyzedTracks >= Math.min(3, a.totalTracks) && a.analyzedTracks * 100 >= minimumCoverage * a.totalTracks;
  const seed = albums.find(a => a.albumId === albumId) ?? null;
  return { seed, seedReady: Boolean(seed && ready(seed)), analyzedAlbums: albums.filter(ready).length, albums: seed && ready(seed) ? albums.filter(a => a.albumId !== albumId && ready(a)).slice(0, 20) : [] };
}
export interface SonicResponse { analyzed: number; total: number; seedReady: boolean; tracks: Track[]; }
export async function sonicMatches(request: SonicRequest): Promise<SonicResponse> {
  if (isTauriRuntime()) return invoke("sonic_matches", { request });
  const album = request.seedAlbumId ? await sonicAlbumMatches(request.seedAlbumId, request.minimumCoverage ?? 50) : null;
  const seed = browserPreview.tracks.find(t => request.seedAlbumId ? t.albumId === request.seedAlbumId : t.trackKey === request.seedKey);
  const excluded = new Set([...request.excludeKeys, request.seedKey]);
  return { analyzed: browserPreview.tracks.filter(t => t.sonicAnalyzed).length, total: browserPreview.summary.songs, seedReady: album ? album.seedReady : Boolean(seed?.sonicAnalyzed),
    tracks: browserPreview.tracks.filter(t => t.sonicAnalyzed && !excluded.has(t.trackKey) && (!request.seedAlbumId || t.albumId !== request.seedAlbumId) && t.loveState !== "banned" && (request.minimumRating === null || (t.rating ?? -1) >= request.minimumRating) && (!request.sameGenre || t.genre === seed?.genre)).slice(0, request.limit) };
}
export async function analyzeSonicSeed(trackKey: string): Promise<void> {
  if (isTauriRuntime()) await invoke("sonic_analyze_seed", { trackKey });
  else { const track = browserPreview.tracks.find(t => t.trackKey === trackKey); if (track) track.sonicAnalyzed = true; }
}
