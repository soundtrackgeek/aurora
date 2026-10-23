import { invoke } from "@tauri-apps/api/core";
import { browserPreview, displayTrackArtist, isTauriRuntime, type Track } from "./library";
export interface JourneyRequest { stopKeys: string[]; connectingTracks: number; minimumRating: number | null; sameGenre: boolean; targetSeconds?: number | null; maxMib?: number | null; albumStopKey?: string | null; }
export interface JourneyTrack { trackId: number; trackKey: string; title: string; artist: string; album: string; albumId: string; albumArtist: string; filePath: string; filename: string; genre: string | null; rating: number | null; seconds: number; loved: boolean; }
export interface JourneyTiming { totalSeconds: number; totalBytes: number; shortfallSeconds: number; albumTrackKeys: string[]; }
export interface JourneyResponse { timing: JourneyTiming | null; stopsReady: boolean[]; analyzed: number; complete: boolean; tracks: JourneyTrack[]; }
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
  if (request.targetSeconds != null) {
    if (!stops.every(Boolean)) throw new Error("Choose available preview stops.");
    const groups = stops.map(t => t!.trackKey === request.albumStopKey ? demo.filter(d => d.albumId === t!.albumId) : [t!]);
    const required = groups.flat();
    if (new Set(required.map(t => t.trackKey)).size !== required.length) throw new Error("Required stops overlap the whole album.");
    let seconds = required.reduce((sum,t) => sum + t.seconds,0);
    const bytesPerSecond = 24000;
    const cap = Math.min(request.targetSeconds, request.maxMib == null ? Infinity : Math.floor(request.maxMib * 1048576 / bytesPerSecond));
    if (seconds > cap) throw new Error("The required stops and whole album exceed your limits.");
    const connectors = pool.filter(t => !required.some(r => r.trackKey === t.trackKey)).filter(t => { if (seconds + t.seconds > cap) return false; seconds += t.seconds; return true; });
    const tracks = groups.flatMap((g,i) => i < groups.length - 1 ? [...g,...connectors.filter((_,n) => n % (groups.length-1) === i)] : g);
    return { analyzed: demo.length, stopsReady: stops.map(Boolean), complete: true, tracks, timing: { totalSeconds: seconds, totalBytes: seconds * bytesPerSecond, shortfallSeconds: request.targetSeconds - seconds, albumTrackKeys: groups.flat().filter(t => groups.some(g => g.length>1 && g.includes(t))).map(t => t.trackKey) } };
  }
  const ready = stops.every(Boolean) && pool.length >= (stops.length - 1) * request.connectingTracks;
  const tracks: JourneyTrack[] = [];
  if (ready) for (let i = 0; i < stops.length; i++) { if (i) tracks.push(...pool.splice(0, request.connectingTracks)); tracks.push(stops[i]!); }
  return { timing: null, analyzed: demo.length, stopsReady: stops.map(Boolean), complete: ready, tracks };
}
export async function saveSonicJourney(request: JourneyRequest, tracks: JourneyTrack[], name: string): Promise<string> {
  if (isTauriRuntime()) { await invoke("sonic_save_journey", { input: { journey: request, trackKeys: tracks.map(t => t.trackKey), name } }); return `Saved ${name} in Music Library Playlists.`; }
  return `Browser preview: saved ${name} (${tracks.length} tracks).`;
}
export function journeyPlaybackTracks(tracks: JourneyTrack[]): Track[] {
  return tracks.map(t => !isTauriRuntime() ? browserPreview.tracks.find(d => d.trackKey === t.trackKey)! : ({ id: String(t.trackId), trackKey: t.trackKey, title: t.title, artist: t.albumArtist, displayArtist: t.artist, album: t.album, albumId: t.albumId, genre: t.genre, rating: t.rating === null ? null : t.rating / 20, durationSeconds: t.seconds, loved: t.loved, loveState: t.loved ? "loved" : "neutral", tagSyncState: null, canUndoTagEdit: false, releaseYear: null, playCount: null }));
}
