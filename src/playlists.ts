import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, type Track } from "./library";

export interface SavedPlaylistSummary {
  id: number;
  name: string;
  description: string;
  trackCount: number;
  updatedAt: string;
}

export interface SavedPlaylistDetail extends Omit<SavedPlaylistSummary, "updatedAt"> {
  missingCount: number;
  tracks: Track[];
}

const selectedPlaylistStorageKey = "aurora:selected-music-library-playlist:v1";

export function loadSelectedPlaylistId(): number | null {
  try {
    const value = Number(window.localStorage.getItem(selectedPlaylistStorageKey));
    return Number.isSafeInteger(value) && value > 0 ? value : null;
  } catch { return null; }
}

export function saveSelectedPlaylistId(id: number): void {
  try { window.localStorage.setItem(selectedPlaylistStorageKey, String(id)); } catch { /* Storage may be unavailable. */ }
}

export async function listMusicLibraryPlaylists(): Promise<SavedPlaylistSummary[]> {
  if (!isTauriRuntime()) return [];
  return invoke<SavedPlaylistSummary[]>("list_music_library_playlists");
}

export async function loadMusicLibraryPlaylist(id: number): Promise<SavedPlaylistDetail> {
  if (!isTauriRuntime()) throw new Error("Saved Music Library playlists are available in the desktop app.");
  return invoke<SavedPlaylistDetail>("music_library_playlist", { id });
}
