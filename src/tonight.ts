import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, type Track } from "./library";
import { loadRatingsOverview, loadRatingAlbumQueue, type RatingAlbum } from "./ratings";

export type TonightIntention = "comfort" | "discovery" | "finish";
export interface TonightRequest { intention: TonightIntention; minutes: number; description: string; useJev: boolean; excludeIds: string[] }
export interface TonightSuggestion { album: RatingAlbum; lastListenedAtMs: number | null; reasons: string[]; feedback: "good" | "dismiss" | null }
export interface TonightResult { request: TonightRequest; suggestions: TonightSuggestion[]; source: "local" | "jev" | "preview"; message: string; model: string | null; generatedAtMs: number; candidateCount: number }
export interface JevSettings { configured: boolean; source: "none" | "vault" | "environment" | "preview"; model: string }
export const defaultTonightRequest: TonightRequest = { intention: "comfort", minutes: 60, description: "", useJev: true, excludeIds: [] };
const feedbackKey = "aurora.tonight.preview-feedback.v1";

function previewFeedback(): Record<string, "good" | "dismiss"> {
  try { return JSON.parse(localStorage.getItem(feedbackKey) ?? "{}"); } catch { return {}; }
}

export async function loadJevSettings(): Promise<JevSettings> {
  return isTauriRuntime() ? invoke("jev_settings") : { configured: false, source: "preview", model: "typesafe/jev-1.13" };
}
export async function saveJevCredentials(request: { mode: "save"; apiKey: string } | { mode: "clear" }): Promise<JevSettings> {
  if (!isTauriRuntime()) throw new Error("Save credentials in the desktop app. Browser preview never stores API keys.");
  return invoke("update_jev_credentials", { request });
}
export async function testJevConnection(): Promise<string> {
  if (!isTauriRuntime()) throw new Error("Test Jev in the desktop app.");
  return invoke("test_jev_connection");
}
export async function requestTonight(request: TonightRequest): Promise<TonightResult> {
  if (isTauriRuntime()) return invoke("tonight_suggestions", { request });
  const overview = await loadRatingsOverview();
  const feedback = previewFeedback();
  const albums = (request.intention === "comfort" ? overview.fiveStarAlbums : overview.initialPage.albums)
    .filter(album => album.durationSeconds <= request.minutes * 60 && !request.excludeIds.includes(album.id) && feedback[`${request.intention}:${album.id}`] !== "dismiss")
    .sort((a, b) => Number(feedback[`${request.intention}:${b.id}`] === "good") - Number(feedback[`${request.intention}:${a.id}`] === "good"));
  return { request, suggestions: albums.slice(0, 3).map(album => ({ album, feedback: feedback[`${request.intention}:${album.id}`] ?? null, lastListenedAtMs: null,
    reasons: [`${Math.floor(album.durationSeconds / 60)}:${String(album.durationSeconds % 60).padStart(2, "0")} · fits your ${request.minutes} minutes`, `${album.ratedTracks} of ${album.totalTracks} tracks rated · ${album.remainingTracks} left`, "Sample library · playback availability is not verified"] })),
    source: "preview", message: "Browser preview with sample albums. Jev and file checks run in the desktop app.", model: null, generatedAtMs: Date.now(), candidateCount: albums.length };
}
export async function saveTonightFeedback(intention: TonightIntention, albumIds: string[], value: "good" | "dismiss"): Promise<void> {
  if (isTauriRuntime()) return invoke("tonight_feedback", { request: { intention, albumIds, value } });
  const feedback = previewFeedback();
  albumIds.forEach(id => { feedback[`${intention}:${id}`] = value; });
  localStorage.setItem(feedbackKey, JSON.stringify(feedback));
}
export async function resetTonightFeedback(): Promise<void> {
  if (isTauriRuntime()) return invoke("tonight_reset_feedback");
  localStorage.removeItem(feedbackKey);
}
export async function loadTonightQueue(album: RatingAlbum, minutes: number): Promise<Track[]> {
  return isTauriRuntime() ? invoke("tonight_album_queue", { albumId: album.id, minutes }) : loadRatingAlbumQueue(album, false, 100);
}

export function tonightAlbumTrack(album: RatingAlbum): Track {
  return { id: `tonight:${album.id}`, trackKey: `tonight:${album.id}`, albumId: album.id, title: album.title, artist: album.artist, album: album.title, releaseYear: album.releaseYear, rating: album.effectiveRating, loved: album.lovedTracks > 0, loveState: "neutral", tagSyncState: null, canUndoTagEdit: false, durationSeconds: album.durationSeconds, genre: album.genre, playCount: null };
}
