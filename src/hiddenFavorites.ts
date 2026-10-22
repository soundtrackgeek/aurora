import { invoke } from "@tauri-apps/api/core";
import { browserPreview, isTauriRuntime } from "./library";
import type { JourneyTrack } from "./sonicJourney";
export interface SonicTrack extends JourneyTrack { distance: number | null }
export interface HiddenSuggestion { track: SonicTrack; anchors: SonicTrack[]; fit: string; broader: boolean; kept: boolean }
export interface HiddenShelf { revision: string; generatedAt: string | null; profile: string; modelVersion: string; modelAuc: number | null; trainingTracks: number; validationTracks: number; historySince: number | null; historyPlays: number; excludedDays: number; note: string; suggestions: HiddenSuggestion[]; kept: SonicTrack[]; message: string | null }
const storageKey = "aurora:hidden-favorites-preview:v1";
function preview(): HiddenShelf {
  const tracks: SonicTrack[] = browserPreview.tracks.slice(0, 9).map(t => ({ trackId: Number(t.id.replace("preview-", "")), trackKey: t.trackKey, title: t.title, artist: t.displayArtist ?? t.artist, albumArtist: t.artist, album: t.album, albumId: t.albumId ?? "", filePath: "Preview", filename: `${t.id}.mp3`, genre: t.genre, rating: t.rating === null ? null : t.rating * 20, seconds: t.durationSeconds ?? 180, loved: t.loved, distance: null }));
  return { revision: String(Date.now()), generatedAt: new Date().toISOString(), profile: "Browser preview", modelVersion: "Sample evidence", modelAuc: null, trainingTracks: 0, validationTracks: 0, historySince: null, historyPlays: 0, excludedDays: 90, note: "Browser preview with sample tracks. The installed app uses Music Library's saved sound analysis and ratings.", suggestions: tracks.slice(2, 7).map((track, i) => ({ track, anchors: tracks.slice(0, 2), fit: "Favorite neighbors", broader: i === 4, kept: false })), kept: [], message: null };
}
export async function operateHiddenFavorites(action: string, revision: string | null = null, trackKey: string | null = null, trackKeys: string[] | null = null): Promise<HiddenShelf> {
  if (isTauriRuntime()) return invoke("hidden_favorites", { request: { action, revision, trackKey, trackKeys } });
  const stored = localStorage.getItem(storageKey);
  let shelf: HiddenShelf = stored ? JSON.parse(stored) : { ...preview(), generatedAt: null, suggestions: [] };
  shelf.message = null;
  if (action === "refresh") shelf = { ...preview(), kept: shelf.kept };
  if (action === "keep") { const item = shelf.suggestions.find(s => s.track.trackKey === trackKey); if (item && !shelf.kept.some(t => t.trackKey === trackKey)) shelf.kept.push(item.track); }
  if (action === "dismiss") shelf.suggestions = shelf.suggestions.filter(s => s.track.trackKey !== trackKey);
  if (action === "remove") shelf.kept = shelf.kept.filter(t => t.trackKey !== trackKey);
  if (action === "save") shelf.message = "Browser preview: saved Hidden favorites — kept for later.";
  shelf.suggestions = shelf.suggestions.map(s => ({ ...s, kept: shelf.kept.some(t => t.trackKey === s.track.trackKey) }));
  localStorage.setItem(storageKey, JSON.stringify(shelf));
  return shelf;
}
