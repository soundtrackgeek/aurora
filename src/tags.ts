import { invoke } from "@tauri-apps/api/core";
import {
  browserPreview,
  currentBrowserPreviewTrack,
  isTauriRuntime,
  updateBrowserPreviewTrack,
  type Track,
} from "./library";

export type LoveState = "neutral" | "loved" | "banned";

export interface TagValues {
  rating: number | null;
  loveState: LoveState;
  releaseYear: number | null;
}

export interface TrackTagState {
  values: TagValues;
  syncState: "pendingImport" | null;
  canUndo: boolean;
}

export interface TrackTagSnapshot {
  track: Track;
  tagState: TrackTagState;
  catalogSync?: CatalogSync;
}

export interface CatalogSync {
  status: "synced" | "pending" | "blocked";
  message?: string | null;
  pendingFolderCount: number;
  blockedFolderCount?: number;
  projectionToken?: number | null;
}

export interface CatalogProjectionDecision {
  accepted: boolean;
  latestToken: number;
}

export interface CatalogTrackProjectionDecision {
  acceptedTrackKeys: ReadonlySet<string>;
  complete: boolean;
  latestToken: number;
  latestTrackTokens: ReadonlyMap<string, number>;
}

export function isCatalogProjectionToken(
  token: number | null | undefined,
): token is number {
  return Number.isSafeInteger(token) && (token ?? 0) > 0;
}

export function advanceCatalogProjectionToken(
  latestToken: number,
  incomingToken: number | null | undefined,
): CatalogProjectionDecision {
  if (incomingToken === null || incomingToken === undefined) {
    return { accepted: true, latestToken };
  }
  if (!isCatalogProjectionToken(incomingToken) || incomingToken <= latestToken) {
    return { accepted: false, latestToken };
  }
  return { accepted: true, latestToken: incomingToken };
}

export function advanceCatalogTrackProjectionTokens(
  latestToken: number,
  latestTrackTokens: ReadonlyMap<string, number>,
  incomingToken: number | null | undefined,
  trackKeys: readonly string[],
): CatalogTrackProjectionDecision {
  const uniqueTrackKeys = [...new Set(trackKeys)];
  if (incomingToken === null || incomingToken === undefined) {
    return {
      acceptedTrackKeys: new Set(uniqueTrackKeys),
      complete: true,
      latestToken,
      latestTrackTokens,
    };
  }
  if (!isCatalogProjectionToken(incomingToken)) {
    return {
      acceptedTrackKeys: new Set(),
      complete: false,
      latestToken,
      latestTrackTokens,
    };
  }

  const acceptedTrackKeys = new Set<string>();
  const nextTrackTokens = new Map(latestTrackTokens);
  for (const trackKey of uniqueTrackKeys) {
    const previousToken = nextTrackTokens.get(trackKey) ?? 0;
    if (incomingToken <= previousToken) continue;
    nextTrackTokens.set(trackKey, incomingToken);
    acceptedTrackKeys.add(trackKey);
  }
  return {
    acceptedTrackKeys,
    complete: acceptedTrackKeys.size === uniqueTrackKeys.length,
    latestToken: Math.max(latestToken, incomingToken),
    latestTrackTokens: nextTrackTokens,
  };
}

export interface TagReconciliationChange {
  trackKey: string;
  values: TagValues;
  syncState: "pendingImport" | null;
}

export interface TagReconciliationIssue {
  trackKey: string;
  message: string;
}

export interface TagReconciliationReport {
  projectionToken: number | null;
  processed: number;
  reconciled: number;
  externalChanges: number;
  catalogCaughtUp: number;
  unchanged: number;
  unavailable: number;
  invalid: number;
  conflicted: number;
  hasMore: boolean;
  changes: TagReconciliationChange[];
  issues: TagReconciliationIssue[];
  catalogSync?: CatalogSync;
}

export type TagEditorTarget =
  | { kind: "track"; trackId: string; trackKey: string; label: string }
  | { kind: "album"; albumId: string; label: string };

export const EDITABLE_TAG_FIELDS = [
  "albumArtist",
  "artist",
  "album",
  "title",
  "genre",
  "publisher",
  "rating",
  "year",
  "releaseYear",
  "trackNumber",
  "trackTotal",
  "discNumber",
  "discTotal",
] as const;

export type EditableTagField = (typeof EDITABLE_TAG_FIELDS)[number];

export interface EditableTagValues {
  albumArtist: string | null;
  artist: string | null;
  album: string | null;
  title: string | null;
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

export interface TagEditorTrackState {
  trackId: string;
  trackKey: string;
  revision: string;
  values: EditableTagValues;
}

export interface TagEditorSnapshot {
  tracks: TagEditorTrackState[];
}

export interface TagEditorUpdateResult {
  state: TagEditorSnapshot;
  tracks: Track[];
  catalogSync?: CatalogSync;
}

export type EditableTagAggregation = {
  [Field in EditableTagField]: {
    value: EditableTagValues[Field];
    mixed: boolean;
  };
};

type PreviewEditableTrack = Track & {
  trackNumber?: number | null;
  trackTotal?: number | null;
  discNumber?: number | null;
  discTotal?: number | null;
};

const browserUndo = new Map<string, Track>();
const browserTracks = new Map<string, Track>();
const browserEditableValues = new Map<string, EditableTagValues>();
const browserTagRevisions = new Map<string, number>();

function nullableText(value: string | null | undefined): string | null {
  return value?.trim() ? value : null;
}

export function editableTagValuesForTrack(track: Track): EditableTagValues {
  const editableTrack = track as PreviewEditableTrack;
  return {
    albumArtist: nullableText(track.artist),
    // Browser preview has no raw TPE1 field, so its display artist is the closest faithful stand-in.
    artist: nullableText(track.displayArtist ?? track.artist),
    album: nullableText(track.album),
    title: nullableText(track.title),
    genre: nullableText(track.genre),
    publisher: nullableText(track.publisher),
    rating: track.rating,
    year: track.originalYear ?? null,
    releaseYear: track.releaseYear,
    trackNumber: editableTrack.trackNumber ?? null,
    trackTotal: editableTrack.trackTotal ?? null,
    discNumber: editableTrack.discNumber ?? null,
    discTotal: editableTrack.discTotal ?? null,
  };
}

export function aggregateEditableTagValues(tracks: TagEditorTrackState[]): EditableTagAggregation {
  return Object.fromEntries(EDITABLE_TAG_FIELDS.map((field) => {
    const first = tracks[0]?.values[field] ?? null;
    return [field, {
      value: first,
      mixed: tracks.some((track) => !Object.is(track.values[field], first)),
    }];
  })) as EditableTagAggregation;
}

function browserValuesForTrack(track: Track): EditableTagValues {
  return browserEditableValues.get(track.id) ?? editableTagValuesForTrack(track);
}

function syncInlineBrowserValues(track: Track): void {
  const values = browserEditableValues.get(track.id);
  if (!values) return;
  browserEditableValues.set(track.id, {
    ...values,
    rating: track.rating,
    releaseYear: track.releaseYear,
  });
}

function bumpBrowserRevision(trackId: string): void {
  browserTagRevisions.set(trackId, (browserTagRevisions.get(trackId) ?? 0) + 1);
}

function browserRevision(track: Track): string {
  return `preview:${track.trackKey}:${browserTagRevisions.get(track.id) ?? 0}`;
}

function browserTracksForTarget(target: TagEditorTarget): Track[] {
  const matches = browserPreview.tracks
    .map((track) => browserTracks.get(track.id) ?? currentBrowserPreviewTrack(track))
    .filter((track) => target.kind === "album"
      ? track.albumId === target.albumId
      : track.id === target.trackId && track.trackKey === target.trackKey);
  if (!matches.length) throw new Error(`${target.kind === "album" ? "Album" : "Track"} is no longer available.`);
  return matches;
}

function browserTagEditorSnapshot(target: TagEditorTarget): TagEditorSnapshot {
  return {
    tracks: browserTracksForTarget(target).map((track) => ({
      trackId: track.id,
      trackKey: track.trackKey,
      revision: browserRevision(track),
      values: { ...browserValuesForTrack(track) },
    })),
  };
}

function trackWithEditableTagValues(
  track: Track,
  fields: readonly EditableTagField[],
  values: EditableTagValues,
): Track {
  const updated: PreviewEditableTrack = { ...track };
  const selected = new Set(fields);
  if (selected.has("albumArtist")) updated.artist = values.albumArtist ?? "";
  if (selected.has("artist")) updated.displayArtist = values.artist ?? "";
  if (selected.has("album")) updated.album = values.album ?? "";
  if (selected.has("title")) updated.title = values.title ?? "";
  if (selected.has("genre")) updated.genre = values.genre;
  if (selected.has("publisher")) updated.publisher = values.publisher;
  if (selected.has("rating")) updated.rating = values.rating;
  if (selected.has("year")) updated.originalYear = values.year;
  if (selected.has("releaseYear")) updated.releaseYear = values.releaseYear;
  if (selected.has("trackNumber")) updated.trackNumber = values.trackNumber;
  if (selected.has("trackTotal")) updated.trackTotal = values.trackTotal;
  if (selected.has("discNumber")) updated.discNumber = values.discNumber;
  if (selected.has("discTotal")) updated.discTotal = values.discTotal;
  updated.tagSyncState = "pendingImport";
  return updated;
}

export async function readTagEditorState(target: TagEditorTarget): Promise<TagEditorSnapshot> {
  if (!isTauriRuntime()) return browserTagEditorSnapshot(target);
  return invoke<TagEditorSnapshot>("tag_editor_state", { target });
}

export async function updateTagEditor(
  target: TagEditorTarget,
  expected: TagEditorSnapshot,
  fields: EditableTagField[],
  values: EditableTagValues,
): Promise<TagEditorUpdateResult> {
  if (!isTauriRuntime()) {
    const current = browserTagEditorSnapshot(target);
    if (JSON.stringify(current) !== JSON.stringify(expected)) {
      throw new Error("One or more preview files changed after the editor opened. Refresh before saving.");
    }
    const updatedTracks = browserTracksForTarget(target).map((track) => {
      const currentValues = browserValuesForTrack(track);
      const desired = { ...currentValues };
      for (const field of fields) {
        (desired[field] as EditableTagValues[typeof field]) = values[field];
      }
      browserEditableValues.set(track.id, desired);
      const updated = {
        ...trackWithEditableTagValues(track, fields, desired),
        // The browser preview models a successful companion receipt immediately.
        tagSyncState: null,
      };
      browserTracks.set(track.id, updated);
      updateBrowserPreviewTrack(updated);
      bumpBrowserRevision(track.id);
      return updated;
    });
    return {
      state: browserTagEditorSnapshot(target),
      tracks: updatedTracks,
      catalogSync: { status: "synced", message: "Music Library updated.", pendingFolderCount: 0 },
    };
  }
  return invoke<TagEditorUpdateResult>("update_tag_editor", {
    request: { target, expected, fields, values },
  });
}

export function tagValuesForTrack(track: Track): TagValues {
  return {
    rating: track.rating,
    loveState: track.loveState,
    releaseYear: track.releaseYear,
  };
}

export function trackWithTagValues(track: Track, values: TagValues): Track {
  return {
    ...track,
    rating: values.rating,
    loveState: values.loveState,
    loved: values.loveState === "loved",
    releaseYear: values.releaseYear,
    tagSyncState: "pendingImport",
    canUndoTagEdit: true,
  };
}

export function trackWithReconciledTags(track: Track, change: TagReconciliationChange): Track {
  return {
    ...track,
    rating: change.values.rating,
    loveState: change.values.loveState,
    loved: change.values.loveState === "loved",
    releaseYear: change.values.releaseYear,
    tagSyncState: change.syncState,
  };
}

function browserSnapshot(track: Track): TrackTagSnapshot {
  const current = browserTracks.get(track.id) ?? track;
  return {
    track: current,
    tagState: {
      values: tagValuesForTrack(current),
      syncState: current.tagSyncState,
      canUndo: current.canUndoTagEdit,
    },
  };
}

export async function readTrackTagState(track: Track): Promise<TrackTagSnapshot> {
  if (!isTauriRuntime()) return browserSnapshot(track);
  return invoke<TrackTagSnapshot>("track_tag_state", { trackId: track.id, trackKey: track.trackKey });
}

export async function updateTrackTags(
  track: Track,
  expected: TagValues,
  desired: TagValues,
): Promise<TrackTagSnapshot> {
  if (!isTauriRuntime()) {
    const current = browserTracks.get(track.id) ?? track;
    if (JSON.stringify(tagValuesForTrack(current)) !== JSON.stringify(expected)) {
      throw new Error("This preview track changed after the editor opened. Reload before saving.");
    }
    browserUndo.set(track.id, current);
    const updated = trackWithTagValues(current, desired);
    browserTracks.set(track.id, updated);
    syncInlineBrowserValues(updated);
    bumpBrowserRevision(track.id);
    updateBrowserPreviewTrack(updated);
    return {
      ...browserSnapshot(updated),
      catalogSync: { status: "synced", message: "Music Library updated.", pendingFolderCount: 0 },
    };
  }
  return invoke<TrackTagSnapshot>("update_track_tags", {
    request: { trackId: track.id, trackKey: track.trackKey, expected, desired },
  });
}

export async function undoTrackTagEdit(track: Track): Promise<TrackTagSnapshot> {
  if (!isTauriRuntime()) {
    const previous = browserUndo.get(track.id);
    if (!previous) throw new Error("There is no preview tag edit to undo.");
    browserUndo.delete(track.id);
    const restored = { ...previous, canUndoTagEdit: false, tagSyncState: null };
    browserTracks.set(track.id, restored);
    syncInlineBrowserValues(restored);
    bumpBrowserRevision(track.id);
    updateBrowserPreviewTrack(restored);
    return {
      ...browserSnapshot(restored),
      catalogSync: { status: "synced", message: "Music Library updated.", pendingFolderCount: 0 },
    };
  }
  return invoke<TrackTagSnapshot>("undo_track_tag_edit", { trackId: track.id, trackKey: track.trackKey });
}

export async function reconcilePendingTags(): Promise<TagReconciliationReport> {
  if (!isTauriRuntime()) {
    return {
      processed: 0,
      reconciled: 0,
      externalChanges: 0,
      catalogCaughtUp: 0,
      unchanged: 0,
      unavailable: 0,
      invalid: 0,
      conflicted: 0,
      hasMore: false,
      changes: [],
      issues: [],
      projectionToken: null,
      catalogSync: { status: "synced", message: "Music Library updated.", pendingFolderCount: 0 },
    };
  }
  return invoke<TagReconciliationReport>("refresh_external_tag_changes");
}
