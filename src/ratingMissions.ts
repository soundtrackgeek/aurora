import { invoke } from "@tauri-apps/api/core";
import { browserPreview, isTauriRuntime } from "./library";
import type { SonicTrack } from "./hiddenFavorites";
export type { SonicTrack } from "./hiddenFavorites";
export interface MissionRequest { action: string; revision: string | null; missionId: string | null; minutes: number | null; mode: string | null; trackKeys: string[] | null }
export interface RatingMission { id: string; kind: string; title: string; reason: string; fit: string; tracks: SonicTrack[]; anchors: SonicTrack[]; seconds: number; albumTotal: number; albumRated: number; completedTracks: number; status: string; startedAt: string | null; completedAt: string | null }
export interface MissionShelf { revision: string; generatedAt: string | null; minutes: number; mode: string; note: string; modelVersion: string; modelAuc: number | null; trainingTracks: number; validationTracks: number; choices: RatingMission[]; active: RatingMission | null; startedMissions: number; completedMissions: number; completedArtists: number; completedNeighborhoods: number; message: string | null }
const storageKey = "aurora:rating-missions-preview:v1";
function empty(): MissionShelf {
  return { revision: "preview-empty", generatedAt: null, minutes: 40, mode: "albums", note: "Browser preview with sample missions. The installed app uses Music Library's saved ratings and sound analysis.", modelVersion: "Sample evidence", modelAuc: null, trainingTracks: 0, validationTracks: 0, choices: [], active: null, startedMissions: 0, completedMissions: 0, completedArtists: 0, completedNeighborhoods: 0, message: null };
}
function demo(shelf: MissionShelf): RatingMission[] {
  return ["fit", "finish", "stretch"].map((kind, i) => {
    const t = browserPreview.tracks[i + 2];
    const track: SonicTrack = { trackId: Number(t.id.replace("preview-", "")), trackKey: t.trackKey, title: t.title, artist: t.displayArtist ?? t.artist, albumArtist: t.artist, album: t.album, albumId: t.albumId ?? "", filePath: "Preview", filename: `${t.id}.mp3`, genre: t.genre, rating: null, seconds: 180, loved: false, distance: null };
    return { id: `${Date.now()}-${kind}`, kind, title: shelf.mode === "mix" ? "A short sample mix" : track.album, reason: "Sample audition choice. Ratings remain your decision.", fit: "Sample sound evidence", tracks: [track], anchors: [], seconds: 180, albumTotal: shelf.mode === "mix" ? 0 : 10, albumRated: shelf.mode === "mix" ? 0 : 9, completedTracks: 0, status: "choice", startedAt: null, completedAt: null };
  });
}
export async function operateRatingMissions(request: MissionRequest): Promise<MissionShelf> {
  if (isTauriRuntime()) return invoke("rating_missions", { request });
  let shelf: MissionShelf;
  try { shelf = JSON.parse(localStorage.getItem(storageKey) ?? "null") ?? empty(); } catch { shelf = empty(); }
  shelf.message = null;
  if (request.action === "refresh") { shelf = { ...shelf, minutes: request.minutes ?? 40, mode: request.mode ?? "albums", generatedAt: new Date().toISOString() }; shelf.choices = demo(shelf); }
  if (request.action === "start") { shelf.active = { ...shelf.choices.find(m => m.id === request.missionId)!, status: "active", startedAt: new Date().toISOString() }; shelf.choices = shelf.choices.filter(m => m.id !== request.missionId); shelf.startedMissions++; }
  if (request.action === "abandon") shelf.active = null;
  if (request.action === "save") shelf.message = "Browser preview: saved the reviewed mission in Playlists.";
  shelf.revision = String(Date.now()); localStorage.setItem(storageKey, JSON.stringify(shelf));
  return shelf;
}
