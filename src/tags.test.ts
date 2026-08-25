import { describe, expect, it } from "vitest";
import { browserPreview, exploreTracks, type Track } from "./library";
import {
  advanceCatalogProjectionToken,
  advanceCatalogTrackProjectionTokens,
  aggregateEditableTagValues,
  editableTagValuesForTrack,
  readTrackTagState,
  reconcilePendingTags,
  tagValuesForTrack,
  trackWithReconciledTags,
  trackWithTagValues,
  undoTrackTagEdit,
  updateTrackTags,
  type EditableTagValues,
  type TagEditorTrackState,
} from "./tags";

describe("catalog projection ordering", () => {
  it("rejects an older edit result delivered after a newer one", () => {
    const newer = advanceCatalogProjectionToken(0, 2);
    const older = advanceCatalogProjectionToken(newer.latestToken, 1);

    expect(newer).toEqual({ accepted: true, latestToken: 2 });
    expect(older).toEqual({ accepted: false, latestToken: 2 });
  });

  it("rejects a reconciliation response delivered after a newer serialized edit", () => {
    const edit = advanceCatalogProjectionToken(0, 8);
    const staleReconciliation = advanceCatalogProjectionToken(edit.latestToken, 7);

    expect(edit).toEqual({ accepted: true, latestToken: 8 });
    expect(staleReconciliation).toEqual({ accepted: false, latestToken: 8 });
  });

  it("accepts an older response for a track not touched by the newer response", () => {
    const newer = advanceCatalogTrackProjectionTokens(0, new Map(), 2, ["track-b"]);
    const olderUnrelated = advanceCatalogTrackProjectionTokens(
      newer.latestToken,
      newer.latestTrackTokens,
      1,
      ["track-a"],
    );

    expect(olderUnrelated.acceptedTrackKeys).toEqual(new Set(["track-a"]));
    expect(olderUnrelated.complete).toBe(true);
    expect(olderUnrelated.latestToken).toBe(2);
  });

  it("filters only tracks already superseded by a newer response", () => {
    const newer = advanceCatalogTrackProjectionTokens(0, new Map(), 4, ["track-b"]);
    const mixedOlder = advanceCatalogTrackProjectionTokens(
      newer.latestToken,
      newer.latestTrackTokens,
      3,
      ["track-a", "track-b"],
    );

    expect(mixedOlder.acceptedTrackKeys).toEqual(new Set(["track-a"]));
    expect(mixedOlder.complete).toBe(false);
  });
});

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
      canUndoTagEdit: false,
    });
  });

  it("saves half-star, love, and Release Year without retaining undo state", async () => {
    const original = track("tag-edit-and-undo");
    const expected = tagValuesForTrack(original);
    const saved = await updateTrackTags(original, expected, {
      rating: 4.5,
      loveState: "loved",
      releaseYear: 2024,
    });

    expect(saved.tagState.values).toEqual({ rating: 4.5, loveState: "loved", releaseYear: 2024 });
    expect(saved.tagState.syncState).toBe("pendingImport");
    expect(saved.tagState.canUndo).toBe(false);
    await expect(undoTrackTagEdit(saved.track)).rejects.toThrow(/cannot be undone/i);
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
    await expect(reconcilePendingTags()).resolves.toMatchObject({
      projectionToken: null,
      processed: 0,
      changes: [],
      hasMore: false,
    });
  });

  it("keeps a saved inline edit when the browser Explorer reloads", async () => {
    const original = browserPreview.tracks[0];
    const desired = { ...tagValuesForTrack(original), rating: 2, loveState: "neutral" as const };
    await updateTrackTags(original, tagValuesForTrack(original), desired);

    const reloaded = await exploreTracks({ artist: original.artist });
    expect(reloaded.items.find((candidate) => candidate.id === original.id)).toMatchObject({
      rating: 2,
      loved: false,
      loveState: "neutral",
      tagSyncState: "pendingImport",
    });
  });
});

function editableValues(overrides: Partial<EditableTagValues> = {}): EditableTagValues {
  return {
    albumArtist: "Five for Fighting",
    artist: "Five for Fighting",
    album: "America Town",
    title: "Superman",
    genre: "Pop Rock",
    publisher: "Aware Records",
    rating: 4.5,
    year: 2000,
    releaseYear: 2000,
    trackNumber: 1,
    trackTotal: 12,
    discNumber: 1,
    discTotal: 1,
    ...overrides,
  };
}

function editorTrack(trackId: string, values: EditableTagValues): TagEditorTrackState {
  return { trackId, trackKey: `c:/music/${trackId}.mp3`, revision: `revision-${trackId}`, values };
}

describe("multi-file tag aggregation", () => {
  it("keeps shared values and marks different values as mixed", () => {
    const aggregated = aggregateEditableTagValues([
      editorTrack("one", editableValues()),
      editorTrack("two", editableValues({ title: "Easy Tonight", trackNumber: 2 })),
    ]);

    expect(aggregated.album).toEqual({ value: "America Town", mixed: false });
    expect(aggregated.genre).toEqual({ value: "Pop Rock", mixed: false });
    expect(aggregated.title).toEqual({ value: "Superman", mixed: true });
    expect(aggregated.trackNumber).toEqual({ value: 1, mixed: true });
  });

  it("projects every editable browser-preview field without changing inline tag helpers", () => {
    const previewTrack = {
      ...track("editable-projection"),
      displayArtist: "Guest vocalist",
      originalYear: 1999,
      publisher: "Aware Records",
      trackNumber: 3,
      trackTotal: 12,
      discNumber: 1,
      discTotal: 2,
    };

    expect(editableTagValuesForTrack(previewTrack)).toMatchObject({
      albumArtist: "Test artist",
      artist: "Guest vocalist",
      publisher: "Aware Records",
      year: 1999,
      trackNumber: 3,
      trackTotal: 12,
      discNumber: 1,
      discTotal: 2,
    });
    expect(tagValuesForTrack(previewTrack)).toEqual({ rating: 3.5, loveState: "neutral", releaseYear: 2001 });
  });
});
