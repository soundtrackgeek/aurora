import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, type Track } from "./library";

export interface SavedPlaylistSummary {
  id: number;
  name: string;
  description: string;
  trackCount: number;
  updatedAt: string;
  smart?: boolean;
}

export interface SavedPlaylistDetail extends Omit<SavedPlaylistSummary, "updatedAt"> {
  missingCount: number;
  tracks: Track[];
  request?: SharedPlaylistRequest | null;
  smartSettings?: SmartPlaylistSettings | null;
  nextCursor?: number | null;
  revision?: string;
}

export interface SharedPlaylistRequest {
  view: "tracks" | "albums";
  searchText: string;
  filters: Record<string, unknown>;
  sort: { field: string; direction: string };
  limit: number;
  offset: number;
}
export interface SmartPlaylistSettings { trackLimit: number; refreshPolicy: "library" | "manual"; }
export interface PlaylistPageContext { id: number; cursor: number | null; revision: string; shuffleSeed?: number; }

export async function saveSmartPlaylist(input: { id: number | null; expectedUpdatedAt?: string; name: string; request: SharedPlaylistRequest; settings: SmartPlaylistSettings }): Promise<{ id: number }> {
  if (!isTauriRuntime()) throw new Error("Shared playlists are saved by Music Library in the desktop app.");
  return invoke("save_smart_playlist", { input });
}
export async function savePlaylistSelection(name: string, tracks: readonly Track[], albumIds: readonly string[]): Promise<{ id: number }> {
  if (!isTauriRuntime()) throw new Error("Shared playlists are saved by Music Library in the desktop app.");
  return invoke("save_playlist_selection", { input: { name, tracks: tracks.map(({ id, trackKey }) => ({ id, trackKey })), albumIds } });
}
export async function refreshSmartPlaylist(id: number): Promise<void> {
  if (!isTauriRuntime()) throw new Error("Refresh shared Smart playlists in the desktop app.");
  await invoke("refresh_smart_playlist", { id });
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

export async function loadMusicLibraryPlaylist(id: number, after: number | null = null, expectedRevision: string | null = null, shuffleSeed: number | null = null): Promise<SavedPlaylistDetail> {
  if (!isTauriRuntime()) throw new Error("Saved Music Library playlists are available in the desktop app.");
  return invoke<SavedPlaylistDetail>("music_library_playlist", { id, after, expectedRevision, shuffleSeed });
}
