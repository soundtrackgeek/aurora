import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "./library";
import type { InboxAlbum, InboxTagApplyRequest } from "./inbox";
import type { CatalogSync, EditableTagField, EditableTagValues } from "./tags";

/** The tags that name an album folder: `Album Artist - Album (Year)`. */
export const FOLDER_NAME_TAG_FIELDS: readonly EditableTagField[] = ["albumArtist", "album", "year"];

export interface LibraryAlbumRenameResult {
  albumId: string;
  sourcePath: string;
  destinationPath: string;
  folderRenamed: boolean;
  renamedFiles: number;
  warnings: string[];
}

export interface LibraryAutoTagRequest {
  albumId: string;
  renameFolder: boolean;
  tags: Omit<InboxTagApplyRequest, "renameAfterApply">;
}

export interface LibraryAutoTagResult {
  changedTracks: number;
  removedTracks: number;
  recoveryPath?: string;
  catalogSync: CatalogSync;
  rename?: LibraryAlbumRenameResult;
  renameError?: string;
}

/**
 * Whether a saved tag edit changes the folder name. Release year, genre and the other fields
 * never rename the folder, and a field saved with its old value is not a change.
 */
export function changesFolderNameTags(
  before: readonly EditableTagValues[],
  fields: readonly EditableTagField[],
  values: EditableTagValues,
): boolean {
  return FOLDER_NAME_TAG_FIELDS.some((field) => fields.includes(field)
    && before.some((track) => normalized(track[field]) !== normalized(values[field])));
}

function normalized(value: string | number | null): string {
  return value === null ? "" : String(value).trim();
}

const AUTO_RENAME_STORAGE_KEY = "aurora.tagEditor.autoRenameFolder";

/** The sidebar editor's "Auto rename folder" choice; on unless the user turned it off. */
export function readAutoRenamePreference(): boolean {
  try {
    return window.localStorage.getItem(AUTO_RENAME_STORAGE_KEY) !== "off";
  } catch {
    return true;
  }
}

export function writeAutoRenamePreference(enabled: boolean): void {
  try {
    window.localStorage.setItem(AUTO_RENAME_STORAGE_KEY, enabled ? "on" : "off");
  } catch {
    // Storage can be unavailable; the choice then lasts for this session only.
  }
}

/** Renames a library album folder and its MP3s after their tags, keeping plays and playlists. */
export async function renameLibraryAlbum(albumId: string): Promise<LibraryAlbumRenameResult> {
  if (!isTauriRuntime()) {
    return { albumId, sourcePath: "", destinationPath: "", folderRenamed: false, renamedFiles: 0, warnings: [] };
  }
  return invoke<LibraryAlbumRenameResult>("rename_library_album", { albumId });
}

/** Reads a library album's files for the auto-tagger. */
export async function loadLibraryAutoTagAlbum(albumId: string): Promise<InboxAlbum> {
  if (!isTauriRuntime()) {
    throw new Error("The library auto-tagger needs the Aurora desktop app.");
  }
  return invoke<InboxAlbum>("library_auto_tag_album", { albumId });
}

export async function applyLibraryAutoTags(request: LibraryAutoTagRequest): Promise<LibraryAutoTagResult> {
  if (!isTauriRuntime()) {
    return {
      changedTracks: request.tags.tracks.length,
      removedTracks: request.tags.removeTrackPaths?.length ?? 0,
      catalogSync: { status: "synced", message: "Music Library updated.", pendingFolderCount: 0 },
    };
  }
  return invoke<LibraryAutoTagResult>("apply_library_auto_tags", { request });
}

/** One status line for a finished rename. */
export function renameSummary(result: LibraryAlbumRenameResult): string | null {
  if (!result.folderRenamed && result.renamedFiles === 0) return null;
  const folder = result.destinationPath.split(/[\\/]/).filter(Boolean).pop() ?? result.destinationPath;
  const files = result.renamedFiles === 1 ? "1 track" : `${result.renamedFiles} tracks`;
  const base = result.folderRenamed ? `Renamed the folder to ${folder} and ${files}.` : `Renamed ${files}.`;
  return result.warnings.length ? `${base} ${result.warnings.join(" ")}` : base;
}
