import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, type Track } from "./library";

export interface SavedPlaylistSummary {
  id: number;
  name: string;
  description: string;
  trackCount: number;
  updatedAt: string;
  smart?: boolean;
  editable?: boolean;
}

export interface SavedPlaylistDetail extends Omit<SavedPlaylistSummary, "updatedAt"> {
  missingCount: number;
  tracks: Track[];
  request?: SharedPlaylistRequest | null;
  smartSettings?: SmartPlaylistSettings | null;
  nextCursor?: number | null;
  revision?: string;
  positions?: number[];
  editable?: boolean;
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

export interface PlaylistImportPreview { path: string; fingerprint: string; name: string; trackCount: number; samples: string[]; }
export interface PlaylistEdit {
  action: "create" | "rename" | "delete" | "append" | "move" | "remove";
  id?: number; expectedUpdatedAt?: string; name?: string; from?: number; to?: number;
  tracks?: readonly Track[]; albumIds?: readonly string[];
}
function requireDesktop() { if (!isTauriRuntime()) throw new Error("Shared playlist authoring requires Aurora desktop and an updated Music Library."); }
export async function authorPlaylist(input: PlaylistEdit): Promise<{ id: number }> {
  requireDesktop();
  return invoke("author_music_library_playlist", { input: { ...input, tracks: input.tracks?.map(({id,trackKey})=>({id,trackKey})) ?? [] } });
}
export async function previewPlaylistImport(): Promise<PlaylistImportPreview | null> { requireDesktop(); return invoke("preview_playlist_import"); }
export async function savePlaylistImport(preview: PlaylistImportPreview, name: string): Promise<{ id: number }> { requireDesktop(); return invoke("save_playlist_import", {path: preview.path, fingerprint: preview.fingerprint, name}); }
export async function exportPlaylist(id: number, revision: string): Promise<boolean> { requireDesktop(); return invoke("export_music_library_playlist", {id,revision}); }

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
