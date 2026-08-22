import { describe, expect, it } from "vitest";
import { browserPreview, exploreTracks, type Track } from "./library";
import {
  readTrackTagState,
  reconcilePendingTags,
  tagValuesForTrack,
  trackWithReconciledTags,
  trackWithTagValues,
  undoTrackTagEdit,
  updateTrackTags,
} from "./tags";

function track(id: string): Track {
  return {
    id,
    trackKey: `preview:${id}`,
    albumId: "album-1",
    title: "Test track",
    artist: "Test artist",
    album: "Test album",
    releaseYear: 2001,
    rating: 3.5,
    loved: false,
    loveState: "neutral",
    tagSyncState: null,
    canUndoTagEdit: false,
    durationSeconds: 180,
    genre: "Test",
    playCount: 0,
  };
}

describe("tag editing preview boundary", () => {
  it("creates the optimistic Explore row without mutating its source", () => {
    const original = track("inline-optimistic");
    const optimistic = trackWithTagValues(original, {
      ...tagValuesForTrack(original),
      rating: 4.5,
      loveState: "loved",
    });

    expect(original.rating).toBe(3.5);
    expect(optimistic).toMatchObject({
      rating: 4.5,
      loved: true,
      loveState: "loved",
      tagSyncState: "pendingImport",
      canUndoTagEdit: true,
    });
  });

  it("saves half-star, love, and Release Year together and supports undo", async () => {
    const original = track("tag-edit-and-undo");
    const expected = tagValuesForTrack(original);
    const saved = await updateTrackTags(original, expected, {
      rating: 4.5,
      loveState: "loved",
      releaseYear: 2024,
    });

    expect(saved.tagState.values).toEqual({ rating: 4.5, loveState: "loved", releaseYear: 2024 });
    expect(saved.tagState.syncState).toBe("pendingImport");
    expect(saved.tagState.canUndo).toBe(true);

    const restored = await undoTrackTagEdit(saved.track);
    expect(restored.tagState.values).toEqual(expected);
    expect(restored.tagState.canUndo).toBe(false);
  });

  it("rejects a stale expected value", async () => {
    const original = track("tag-edit-conflict");
    await updateTrackTags(original, tagValuesForTrack(original), {
      rating: 5,
      loveState: "banned",
      releaseYear: 2001,
    });

    await expect(
      updateTrackTags(original, tagValuesForTrack(original), {
        rating: 1,
        loveState: "neutral",
        releaseYear: 1999,
      }),
    ).rejects.toThrow(/changed after the editor opened/i);
    expect((await readTrackTagState(original)).tagState.values.loveState).toBe("banned");
  });

  it("applies an authoritative external reconciliation without changing undo availability", () => {
    const original = { ...track("external-change"), canUndoTagEdit: true, tagSyncState: "pendingImport" as const };
    const reconciled = trackWithReconciledTags(original, {
      trackKey: original.trackKey,
      values: { rating: 4, loveState: "loved", releaseYear: 2002 },
      syncState: null,
    });
    expect(reconciled).toMatchObject({
      rating: 4,
      loved: true,
      releaseYear: 2002,
      tagSyncState: null,
      canUndoTagEdit: true,
    });
  });

  it("does no filesystem reconciliation in browser preview", async () => {
    await expect(reconcilePendingTags()).resolves.toMatchObject({ processed: 0, changes: [], hasMore: false });
  });

  it("keeps a saved inline edit when the browser Explorer reloads", async () => {
    const original = browserPreview.tracks[0];
    const desired = { ...tagValuesForTrack(original), rating: 2, loveState: "neutral" as const };
    const saved = await updateTrackTags(original, tagValuesForTrack(original), desired);

    const reloaded = await exploreTracks({ artist: original.artist });
    expect(reloaded.items.find((candidate) => candidate.id === original.id)).toMatchObject({
      rating: 2,
      loved: false,
      loveState: "neutral",
      tagSyncState: "pendingImport",
    });

    await undoTrackTagEdit(saved.track);
  });
});
