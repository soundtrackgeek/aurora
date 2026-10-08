import { invoke } from "@tauri-apps/api/core";
import { browserPreview, displayTrackArtist, isTauriRuntime, type Track } from "./library";
export interface JourneyRequest { stopKeys: string[]; connectingTracks: number; minimumRating: number | null; sameGenre: boolean; }
export interface JourneyTrack { trackId: number; trackKey: string; title: string; artist: string; album: string; albumId: string; albumArtist: string; filePath: string; filename: string; genre: string | null; rating: number | null; seconds: number; loved: boolean; }
export interface JourneyResponse { stopsReady: boolean[]; analyzed: number; complete: boolean; tracks: JourneyTrack[]; }
function demoTracks(): JourneyTrack[] { return browserPreview.tracks.filter(t => t.loveState !== "banned").map(t => ({ trackId: Number(t.id.replace("preview-", "")), trackKey: t.trackKey, title: t.title, artist: displayTrackArtist(t), album: t.album, albumId: t.albumId ?? "", albumArtist: t.artist, filePath: "Preview", filename: `${t.id}.mp3`, genre: t.genre, rating: t.rating === null ? null : t.rating * 20, seconds: t.durationSeconds ?? 0, loved: t.loved })); }
export async function searchJourneyTracks(text: string): Promise<JourneyTrack[]> {
  if (isTauriRuntime()) return invoke("sonic_journey_search", { text });
  const query = text.trim().toLowerCase();
  return query.length < 2 ? [] : demoTracks().filter(t => `${t.artist} ${t.title}`.toLowerCase().includes(query)).slice(0, 20);
}
export async function buildSonicJourney(request: JourneyRequest): Promise<JourneyResponse> {
  if (isTauriRuntime()) return invoke("sonic_journey", { request });
  const demo = demoTracks(), stops = request.stopKeys.map(key => demo.find(t => t.trackKey === key));
  const pool = demo.filter(t => !request.stopKeys.includes(t.trackKey) && (request.minimumRating === null || (t.rating ?? -1) >= request.minimumRating) && (!request.sameGenre || t.genre === stops[0]?.genre));
  const ready = stops.every(Boolean) && pool.length >= (stops.length - 1) * request.connectingTracks;
  const tracks: JourneyTrack[] = [];
  if (ready) for (let i = 0; i < stops.length; i++) { if (i) tracks.push(...pool.splice(0, request.connectingTracks)); tracks.push(stops[i]!); }
  return { analyzed: demo.length, stopsReady: stops.map(Boolean), complete: ready, tracks };
}
export async function saveSonicJourney(request: JourneyRequest, tracks: JourneyTrack[], name: string): Promise<string> {
  if (isTauriRuntime()) { await invoke("sonic_save_journey", { input: { journey: request, trackKeys: tracks.map(t => t.trackKey), name } }); return `Saved ${name} in Music Library Playlists.`; }
  return `Browser preview: saved ${name} (${tracks.length} tracks).`;
}
export function journeyPlaybackTracks(tracks: JourneyTrack[]): Track[] {
  return tracks.map(t => !isTauriRuntime() ? browserPreview.tracks.find(d => d.trackKey === t.trackKey)! : ({ id: String(t.trackId), trackKey: t.trackKey, title: t.title, artist: t.albumArtist, displayArtist: t.artist, album: t.album, albumId: t.albumId, genre: t.genre, rating: t.rating === null ? null : t.rating / 20, durationSeconds: t.seconds, loved: t.loved, loveState: t.loved ? "loved" : "neutral", tagSyncState: null, canUndoTagEdit: false, releaseYear: null, playCount: null }));
}
