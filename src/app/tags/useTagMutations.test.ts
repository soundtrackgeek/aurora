import { act, cleanup, renderHook } from "@testing-library/react";
import type { SetStateAction } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { browserPreview, type LibrarySnapshot, type Track } from "../../library";
import * as tags from "../../tags";
import { defaultExplorerFilters } from "../../viewPreferences";
import { useTagMutations, type TagMutationOptions } from "./useTagMutations";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

function state<T>(initial: T) {
  let value = initial;
  return {
    get value() { return value; },
    set: vi.fn((next: SetStateAction<T>) => {
      value = typeof next === "function" ? (next as (current: T) => T)(value) : next;
    }),
  };
}

function options() {
  const track: Track = { ...browserPreview.tracks[0], rating: null, loved: false, loveState: "neutral" };
  const snapshot = state<LibrarySnapshot | null>({ ...browserPreview, tracks: [track] });
  const explorerTracks = state([track]);
  const latestTrackProjectionTokensRef = { current: new Map<string, number>() as ReadonlyMap<string, number> };
  let latestToken = 0;
  const playback = { refreshTrack: vi.fn(), applySnapshot: vi.fn() };
  const input: TagMutationOptions = {
    snapshot: snapshot.value,
    setSnapshot: snapshot.set,
    selectedTrack: track,
    setSelectedTrack: state<Track | null>(track).set,
    activeNav: "Songs",
    setSyncMessage: state<string | null>(null).set,
    explorer: {
      explorerTracks: explorerTracks.value,
      explorerFilters: defaultExplorerFilters,
      setExplorerTracks: explorerTracks.set,
      setExplorerAlbums: vi.fn(),
      setExplorerReloadToken: vi.fn(),
      preserveExplorerOnReloadRef: { current: false },
      pendingExplorerAlbumIdRef: { current: null },
    },
    inspector: {
      selectedAlbumId: null, albumTracks: [], setAlbumTracks: vi.fn(),
      setAlbumTracksTruncated: vi.fn(), setAlbumDetailState: vi.fn(), albumRequestRef: { current: 0 },
    },
    workspace: { mainScrollRef: { current: null }, scrollPositionByDestinationRef: { current: {} }, restoringScrollRef: { current: false } },
    genres: { setGenreDetail: vi.fn(), setGenreAtlasGenres: vi.fn(), setGenreIndexReloadToken: vi.fn(), setGenreDetailReloadToken: vi.fn() },
    years: { yearAlbumTracks: [], setYearAlbumTracks: vi.fn() },
    ratings: { ratingAlbumTracks: [], setRatingAlbumTracks: vi.fn() },
    publishers: { publisherAlbumTracks: [], setPublisherAlbumTracks: vi.fn() },
    projection: {
      latestTrackProjectionTokensRef,
      acceptTrackProjectionKeys: (keys, token) => {
        const decision = tags.advanceCatalogTrackProjectionTokens(latestToken, latestTrackProjectionTokensRef.current, token, keys);
        latestToken = decision.latestToken;
        latestTrackProjectionTokensRef.current = decision.latestTrackTokens;
        return decision;
      },
    },
    handleCatalogSync: vi.fn(async () => false),
    playback,
  };
  return { input, track, explorerTracks, snapshot, playback };
}

it.each([false, true])("guards duplicate saves and rolls back only without a newer projection (newer: %s)", async (hasNewerProjection) => {
  const fixture = options();
  let reject!: (error: Error) => void;
  const update = vi.spyOn(tags, "updateTrackTags").mockReturnValue(new Promise((_, fail) => { reject = fail; }));
  const { result } = renderHook(() => useTagMutations(fixture.input));
  let saving!: Promise<void>;
  const desired = { ...tags.tagValuesForTrack(fixture.track), rating: 4 };
  await act(async () => {
    saving = result.current.saveInlineTagChange(fixture.track, desired);
    await result.current.saveInlineTagChange(fixture.track, desired);
  });
  expect(update).toHaveBeenCalledTimes(1);
  expect(result.current.inlineSavingKeys.has(fixture.track.trackKey)).toBe(true);
  expect(fixture.explorerTracks.value[0].rating).toBe(4);
  if (hasNewerProjection) {
    await act(async () => {
      result.current.applyTrackChanges([{ ...fixture.track, rating: 5 }], { status: "synced", pendingFolderCount: 0, projectionToken: 10 });
    });
  }
  await act(async () => { reject(new Error("file locked")); await saving; });
  expect(fixture.explorerTracks.value[0].rating).toBe(hasNewerProjection ? 5 : null);
  expect(result.current.inlineSavingKeys.size).toBe(0);
  expect(fixture.input.setSyncMessage).toHaveBeenLastCalledWith(`Could not save ${fixture.track.title}: file locked`);
});

it("ignores a late tag-save result after a newer track projection", async () => {
  const fixture = options();
  let resolve!: (snapshot: tags.TrackTagSnapshot) => void;
  vi.spyOn(tags, "updateTrackTags").mockReturnValue(new Promise((finish) => { resolve = finish; }));
  const { result } = renderHook(() => useTagMutations(fixture.input));
  let saving!: Promise<void>;
  const desired = { ...tags.tagValuesForTrack(fixture.track), rating: 3 };
  await act(async () => { saving = result.current.saveInlineTagChange(fixture.track, desired); });
  await act(async () => {
    result.current.applyTrackChanges([{ ...fixture.track, rating: 5 }], { status: "synced", pendingFolderCount: 0, projectionToken: 10 });
    resolve({
      track: { ...fixture.track, rating: 3 },
      tagState: { values: desired, syncState: null, canUndo: true },
      catalogSync: { status: "synced", pendingFolderCount: 0, projectionToken: 9 },
    });
    await saving;
  });
  expect(fixture.explorerTracks.value[0].rating).toBe(5);
  expect(fixture.input.handleCatalogSync).not.toHaveBeenCalled();
  expect(result.current.inlineTagRevisions[fixture.track.trackKey]).toBeUndefined();
  expect(result.current.inlineSavingKeys.size).toBe(0);
});
