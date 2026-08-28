import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "./library";
import type { EditableTagField, EditableTagValues } from "./tags";

export interface InboxSettingsStatus {
  monitoredFolders: string[];
  discogsConfigured: boolean;
  discogsAuthMode: "token" | "consumer" | null;
  discogsIncompleteConsumerKey: boolean;
  warning: string | null;
}

export interface InboxTrack {
  path: string;
  fileName: string;
  albumArtist: string | null;
  title: string | null;
  artist: string | null;
  album: string | null;
  genre: string | null;
  publisher: string | null;
  rating: number | null;
  year: number | null;
  releaseYear: number | null;
  trackNumber: number | null;
  trackTotal: number | null;
  discNumber: number | null;
  discTotal: number | null;
}

export interface InboxAlbum {
  id: string;
  path: string;
  folderName: string;
  artist: string | null;
  album: string | null;
  genre: string | null;
  publisher: string | null;
  year: number | null;
  trackCount: number;
  artworkPresent: boolean;
  modifiedAtMs: number;
  readiness: { ready: boolean; issues: string[] };
  tracks: InboxTrack[];
}

export interface InboxSnapshot {
  settings: InboxSettingsStatus;
  albums: InboxAlbum[];
  scannedAtMs: number;
}

export type MetadataSource = "musicbrainz" | "discogs";

export interface ReleaseCandidate {
  source: MetadataSource;
  id: string;
  score: number;
  title: string;
  artist: string;
  year: number | null;
  country: string | null;
  format: string | null;
  publisher: string | null;
  trackCount: number | null;
  coverUrl: string | null;
}

export interface ReleaseTrack {
  title: string;
  artist: string | null;
  trackNumber: number | null;
  trackTotal: number | null;
  discNumber: number | null;
  discTotal: number | null;
  durationMs: number | null;
}

export interface ReleaseCandidateDetail {
  candidate: ReleaseCandidate;
  albumArtist: string | null;
  album: string | null;
  genre: string | null;
  publisher: string | null;
  year: number | null;
  discTotal: number | null;
  tracks: ReleaseTrack[];
}

export interface ReleaseSearchResult {
  candidates: ReleaseCandidate[];
  discogsConfigured: boolean;
  warnings: string[];
}

export interface InboxTagApplyRequest {
  albumPath: string;
  fields: EditableTagField[];
  tracks: Array<{ path: string; values: EditableTagValues }>;
  renameAfterApply: boolean;
}

export interface InboxRenameResult {
  albumPath: string;
  renamedTracks: number;
  folderRenamed: boolean;
}

export interface InboxBatchRenameResult {
  renamedTracks: number;
  renamedAlbums: number;
  renamedFolders: number;
  failures: Array<{ albumPath: string; message: string }>;
}

export function inboxCoverUrl(
  album: Pick<InboxAlbum, "id" | "artworkPresent" | "modifiedAtMs" | "tracks">,
  size: 64 | 128 | 256,
): string | null {
  if (!album.artworkPresent || !album.tracks[0]) return null;
  if (!isTauriRuntime()) {
    return album.id === "preview-freak"
      ? `/__aurora-preview-cover/preview-freak?size=${size}`
      : null;
  }
  return `http://aurora-cover.localhost/inbox/${encodeURIComponent(album.tracks[0].path)}?size=${size}&revision=${album.modifiedAtMs}`;
}

const previewTracks = [
  "Memories Calling", "Kahlua Confusion", "Dying Alone", "Don’t Stop Running", "Without You",
  "Straight Line", "Day to Come", "What It Is", "Fly So Gently", "Shadows",
].map((title, index): InboxTrack => ({
  path: `C:\\Music\\Inbox\\Baltimoore - Freak\\${String(index + 1).padStart(2, "0")} - ${title}.mp3`,
  fileName: `${String(index + 1).padStart(2, "0")} - ${title}.mp3`,
  albumArtist: "Baltimoore",
  title,
  artist: "Baltimoore",
  album: "Freak",
  genre: null,
  publisher: "SPV Records",
  rating: null,
  year: 1990,
  releaseYear: 1990,
  trackNumber: index + 1,
  trackTotal: 10,
  discNumber: null,
  discTotal: null,
}));

let previewSettings: InboxSettingsStatus = {
  monitoredFolders: ["C:\\Music\\Inbox", "D:\\Bandcamp"],
  discogsConfigured: true,
  discogsAuthMode: "token",
  discogsIncompleteConsumerKey: false,
  warning: null,
};

const previewAlbum: InboxAlbum = {
  id: "preview-freak",
  path: "C:\\Music\\Inbox\\Baltimoore - Freak",
  folderName: "Baltimoore - Freak",
  artist: "Baltimoore",
  album: "Freak",
  genre: null,
  publisher: "SPV Records",
  year: 1990,
  trackCount: 10,
  artworkPresent: true,
  modifiedAtMs: Date.now() - 5 * 60_000,
  readiness: {
    ready: false,
    issues: [
      "Genre is missing",
      "Album folder is not organized as Album Artist - Album (Year)",
      "One or more track filenames are not organized from their tags",
    ],
  },
  tracks: previewTracks,
};

const previewCandidates: ReleaseCandidate[] = [
  { source: "musicbrainz", id: "mb-freak-de", score: 100, title: "Freak", artist: "Baltimoore", year: 1990, country: "DE", format: "CD", publisher: "SPV Records", trackCount: 10, coverUrl: null },
  { source: "discogs", id: "discogs-freak-de", score: 94, title: "Freak", artist: "Baltimoore", year: 1990, country: "DE", format: "CD", publisher: "SPV Records", trackCount: 10, coverUrl: null },
  { source: "discogs", id: "discogs-freak-se", score: 82, title: "Freak", artist: "Baltimoore", year: 1990, country: "SE", format: "CD", publisher: "V.I.P. Records", trackCount: 10, coverUrl: null },
];

function previewDetail(candidate: ReleaseCandidate): ReleaseCandidateDetail {
  return {
    candidate,
    albumArtist: candidate.artist,
    album: candidate.title,
    genre: "Hard Rock",
    publisher: candidate.publisher,
    year: candidate.year,
    discTotal: 1,
    tracks: previewTracks.map((track) => ({
      title: track.title ?? track.fileName,
      artist: "Baltimoore",
      trackNumber: track.trackNumber,
      trackTotal: track.trackTotal,
      discNumber: 1,
      discTotal: 1,
      durationMs: null,
    })),
  };
}

export async function loadInboxSnapshot(): Promise<InboxSnapshot> {
  if (!isTauriRuntime()) return { settings: previewSettings, albums: [previewAlbum], scannedAtMs: Date.now() };
  return invoke<InboxSnapshot>("inbox_snapshot");
}

export async function loadInboxSettings(): Promise<InboxSettingsStatus> {
  if (!isTauriRuntime()) return previewSettings;
  return invoke<InboxSettingsStatus>("inbox_settings");
}

export async function selectInboxMonitorFolder(): Promise<string | null> {
  if (!isTauriRuntime()) return "E:\\New Music";
  return invoke<string | null>("select_inbox_monitor_folder");
}

export async function addInboxMonitorFolder(folder: string): Promise<InboxSettingsStatus> {
  if (!isTauriRuntime()) {
    if (!previewSettings.monitoredFolders.includes(folder)) previewSettings = { ...previewSettings, monitoredFolders: [...previewSettings.monitoredFolders, folder] };
    return previewSettings;
  }
  return invoke<InboxSettingsStatus>("add_inbox_monitor_folder", { folder });
}

export async function removeInboxMonitorFolder(folder: string): Promise<InboxSettingsStatus> {
  if (!isTauriRuntime()) {
    previewSettings = { ...previewSettings, monitoredFolders: previewSettings.monitoredFolders.filter((value) => value !== folder) };
    return previewSettings;
  }
  return invoke<InboxSettingsStatus>("remove_inbox_monitor_folder", { folder });
}

export type DiscogsCredentialsRequest =
  | { mode: "token"; token: string }
  | { mode: "consumer"; consumerKey: string; consumerSecret: string }
  | { mode: "clear" };

export async function updateDiscogsCredentials(request: DiscogsCredentialsRequest): Promise<InboxSettingsStatus> {
  if (!isTauriRuntime()) {
    previewSettings = {
      ...previewSettings,
      discogsConfigured: request.mode !== "clear",
      discogsAuthMode: request.mode === "clear" ? null : request.mode,
      discogsIncompleteConsumerKey: false,
    };
    return previewSettings;
  }
  return invoke<InboxSettingsStatus>("update_discogs_credentials", { request });
}

export async function searchInboxReleases(artist: string, album: string, trackCount: number): Promise<ReleaseSearchResult> {
  if (!isTauriRuntime()) return { candidates: previewCandidates, discogsConfigured: previewSettings.discogsConfigured, warnings: [] };
  return invoke<ReleaseSearchResult>("search_inbox_releases", { request: { artist, album, trackCount } });
}

export async function loadInboxReleaseDetail(candidate: ReleaseCandidate): Promise<ReleaseCandidateDetail> {
  if (!isTauriRuntime()) return previewDetail(candidate);
  return invoke<ReleaseCandidateDetail>("inbox_release_detail", { request: { source: candidate.source, id: candidate.id } });
}

export async function applyInboxTags(request: InboxTagApplyRequest): Promise<{ changedTracks: number; renamedTracks: number; albumPath: string }> {
  if (!isTauriRuntime()) return { changedTracks: request.tracks.length, renamedTracks: request.renameAfterApply ? request.tracks.length : 0, albumPath: request.albumPath };
  return invoke<{ changedTracks: number; renamedTracks: number; albumPath: string }>("apply_inbox_tags", { request });
}

export async function renameInboxAlbum(albumPath: string): Promise<InboxRenameResult> {
  if (!isTauriRuntime()) return { albumPath: `${albumPath} (renamed)`, renamedTracks: previewTracks.length, folderRenamed: true };
  return invoke<InboxRenameResult>("rename_inbox_album", { request: { albumPath } });
}

export async function renameInboxAlbums(albumPaths: string[]): Promise<InboxBatchRenameResult> {
  if (!isTauriRuntime()) {
    return {
      renamedTracks: previewTracks.length * albumPaths.length,
      renamedAlbums: albumPaths.length,
      renamedFolders: albumPaths.length,
      failures: [],
    };
  }
  return invoke<InboxBatchRenameResult>("rename_inbox_albums", { request: { albumPaths } });
}
