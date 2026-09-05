import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./library";

export type LibraryIntakeCategoryId = "general" | "scores" | "synthwave";
export type LibraryIntakeAction = "add" | "replace" | "remove";

export interface LibraryIntakeCategory {
  id: LibraryIntakeCategoryId;
  label: string;
  destinationRoot: string;
  available: boolean;
}

export interface LibraryBridgeCapabilities {
  bridgeVersion: number;
  categories: LibraryIntakeCategory[];
  supports: {
    singleAlbum: boolean;
    batchFolders: boolean;
    crossVolumeCopy: boolean;
    previewRequired: boolean;
    replaceExistingAlbums: boolean;
    moveAlbumsToInbox: boolean;
  };
}

export interface LibraryIntakeDelta {
  addedTracks: number;
  changedTracks: number;
  removedTracks: number;
  addedAlbums: number;
  changedAlbums: number;
  removedAlbums: number;
}

export interface LibraryIntakeAlbumPreview {
  sourcePath: string;
  destinationPath: string;
  artist: string;
  album: string;
  year: string;
  trackCount: number;
  action: LibraryIntakeAction;
  existingTrackCount: number;
  matchedTrackCount: number;
  existingRatedTrackCount: number;
  existingLovedTrackCount: number;
}

export interface LibraryIntakePreview {
  planId: string;
  sessionId: number;
  sourcePath: string;
  category: Omit<LibraryIntakeCategory, "available"> | {
    id: "inbox";
    label: string;
    destinationRoot: string;
  };
  albumCount: number;
  trackCount: number;
  delta: LibraryIntakeDelta;
  albums: LibraryIntakeAlbumPreview[];
  canApply: boolean;
  suspiciousFlags?: string[];
  conflicts?: string[];
  errors?: string[];
}

export interface LibraryIntakeApplyAlbum {
  sourcePath: string;
  destinationPath: string;
  action: LibraryIntakeAction;
  recoveryPath: string | null;
  cleanupStatus: "removed" | "retained";
}

export interface LibraryIntakeApplyResult {
  planId: string;
  sessionId: number;
  status: "completed" | "completedWithWarnings";
  albumCount: number;
  trackCount: number;
  movedAlbumCount: number;
  importRunId: number;
  backupPath: string | null;
  albums: LibraryIntakeApplyAlbum[];
  cleanupWarnings: string[];
}

export interface LibraryIntakeProgress {
  operation: "previewBatch" | "applyBatch" | string;
  stage: string;
  message: string;
  completedAlbums: number;
  totalAlbums: number;
  processedFiles: number;
  totalFiles: number;
  processedBytes: number;
  totalBytes: number;
}

export interface LibraryIntakePreviewRequest {
  sourcePath: string;
  category: LibraryIntakeCategoryId;
}

export interface LibraryIntakeApplyRequest {
  planId: string;
  sessionId: number;
}

export interface LibraryMoveToInboxPreviewRequest {
  albumId: string;
  inboxPath: string;
}

export interface LibraryIntakeAdapter {
  capabilities: () => Promise<LibraryBridgeCapabilities>;
  selectFolder: () => Promise<string | null>;
  preview: (request: LibraryIntakePreviewRequest) => Promise<LibraryIntakePreview>;
  previewMoveToInbox: (request: LibraryMoveToInboxPreviewRequest) => Promise<LibraryIntakePreview>;
  apply: (request: LibraryIntakeApplyRequest) => Promise<LibraryIntakeApplyResult>;
}

export const libraryIntakeCategories: ReadonlyArray<{
  id: LibraryIntakeCategoryId;
  label: string;
  description: string;
}> = [
  {
    id: "general",
    label: "General music",
    description: "Albums outside the score and synthwave collections.",
  },
  {
    id: "scores",
    label: "Movie / TV / game music",
    description: "Film, television, animation, and game scores.",
  },
  {
    id: "synthwave",
    label: "Synthwave",
    description: "The dedicated synthwave collection.",
  },
] as const;

const browserCapabilities: LibraryBridgeCapabilities = {
  bridgeVersion: 0,
  categories: libraryIntakeCategories.map(({ id, label }) => ({
    id,
    label,
    destinationRoot: "Native Aurora required",
    available: false,
  })),
  supports: {
    singleAlbum: false,
    batchFolders: false,
    crossVolumeCopy: false,
    previewRequired: true,
    replaceExistingAlbums: false,
    moveAlbumsToInbox: false,
  },
};

export async function loadLibraryBridgeCapabilities(): Promise<LibraryBridgeCapabilities> {
  if (!isTauriRuntime()) return browserCapabilities;
  return invoke<LibraryBridgeCapabilities>("library_bridge_capabilities");
}

export async function selectLibraryIntakeFolder(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  return invoke<string | null>("select_library_intake_folder");
}

export async function previewLibraryIntakeBatch(
  request: LibraryIntakePreviewRequest,
): Promise<LibraryIntakePreview> {
  if (!isTauriRuntime()) {
    throw new Error("Adding music is available in the native Aurora app.");
  }
  return invoke<LibraryIntakePreview>("preview_library_intake_batch", { request });
}

export async function applyLibraryIntakeBatch(
  request: LibraryIntakeApplyRequest,
): Promise<LibraryIntakeApplyResult> {
  if (!isTauriRuntime()) {
    throw new Error("Adding music is available in the native Aurora app.");
  }
  return invoke<LibraryIntakeApplyResult>("apply_library_intake_batch", { request });
}

export async function previewLibraryMoveToInbox(
  request: LibraryMoveToInboxPreviewRequest,
): Promise<LibraryIntakePreview> {
  if (!isTauriRuntime()) {
    throw new Error("Moving an album back to Inbox is available in the native Aurora app.");
  }
  return invoke<LibraryIntakePreview>("preview_library_move_to_inbox", { request });
}

export async function listenLibraryIntakeProgress(
  onProgress: (progress: LibraryIntakeProgress) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => {};
  return listen<LibraryIntakeProgress>("library-intake-progress", (event) => {
    onProgress(event.payload);
  });
}

export const libraryIntakeAdapter: LibraryIntakeAdapter = {
  capabilities: loadLibraryBridgeCapabilities,
  selectFolder: selectLibraryIntakeFolder,
  preview: previewLibraryIntakeBatch,
  previewMoveToInbox: previewLibraryMoveToInbox,
  apply: applyLibraryIntakeBatch,
};

export async function previewLibraryRemoveAlbum(albumId: string): Promise<LibraryIntakePreview> {
  if (!isTauriRuntime()) throw new Error("Removing an album is available in the native Aurora app.");
  return invoke<LibraryIntakePreview>("preview_library_remove_album", { albumId });
}
