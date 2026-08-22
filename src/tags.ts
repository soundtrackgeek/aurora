import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, updateBrowserPreviewTrack, type Track } from "./library";

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
}

const browserUndo = new Map<string, Track>();
const browserTracks = new Map<string, Track>();

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
    updateBrowserPreviewTrack(updated);
    return browserSnapshot(updated);
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
    updateBrowserPreviewTrack(restored);
    return browserSnapshot(restored);
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
    };
  }
  return invoke<TagReconciliationReport>("refresh_external_tag_changes");
}
