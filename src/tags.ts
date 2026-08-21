import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, type Track } from "./library";

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

const browserUndo = new Map<string, Track>();
const browserTracks = new Map<string, Track>();

export function tagValuesForTrack(track: Track): TagValues {
  return {
    rating: track.rating,
    loveState: track.loveState,
    releaseYear: track.releaseYear,
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
    const updated: Track = {
      ...current,
      rating: desired.rating,
      loveState: desired.loveState,
      loved: desired.loveState === "loved",
      releaseYear: desired.releaseYear,
      tagSyncState: "pendingImport",
      canUndoTagEdit: true,
    };
    browserTracks.set(track.id, updated);
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
    return browserSnapshot(restored);
  }
  return invoke<TrackTagSnapshot>("undo_track_tag_edit", { trackId: track.id, trackKey: track.trackKey });
}
