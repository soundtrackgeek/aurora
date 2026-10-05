import { act, cleanup, renderHook } from "@testing-library/react";
import type { SetStateAction } from "react";
import { afterEach, expect, it, vi } from "vitest";
import * as library from "../../library";
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
      selectedAlbumId: null, selectedAlbumIdRef: { current: null }, albumTracks: [], setAlbumTracks: vi.fn(),
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

it.each([
  ["save", "another album"], ["save", "closed details"],
  ["catalog sync", "another album"], ["catalog sync", "closed details"],
])("preserves %s-time navigation to %s when an inline rating finishes", async (stage, destination) => {
  const fixture = options();
  const older = await library.loadAlbumDetail(fixture.track.albumId!);
  const newer = await library.loadAlbumDetail("preview-viva");
  const albumTracks = state(older.tracks);
  const selectedTrack = state<Track | null>(fixture.track);
  const selectedAlbumIdRef = { current: older.album.id as string | null };
  Object.assign(fixture.input.inspector, {
    selectedAlbumId: older.album.id, selectedAlbumIdRef, albumTracks: albumTracks.value,
    setAlbumTracks: albumTracks.set,
  });
  fixture.input.setSelectedTrack = selectedTrack.set;
  const desired = { ...tags.tagValuesForTrack(fixture.track), rating: 4 };
  const saved: tags.TrackTagSnapshot = {
    track: { ...fixture.track, rating: 4 },
    tagState: { values: desired, syncState: null, canUndo: false },
    catalogSync: { status: "synced", pendingFolderCount: 0 },
  };
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => { finish = resolve; });
  vi.spyOn(tags, "updateTrackTags").mockImplementation(async () => {
    if (stage === "save") await pending;
    return saved;
  });
  fixture.input.handleCatalogSync = vi.fn(async () => {
    if (stage === "catalog sync") await pending;
    return false;
  });
  const load = vi.spyOn(library, "loadAlbumDetail").mockResolvedValue(older);
  const { result, rerender } = renderHook(({ input }) => useTagMutations(input), { initialProps: { input: fixture.input } });
  let saving!: Promise<void>;
  await act(async () => { saving = result.current.saveInlineTagChange(fixture.track, desired); });
  const currentTracks = destination === "another album" ? newer.tracks : [];
  selectedAlbumIdRef.current = destination === "another album" ? newer.album.id : null;
  fixture.input.inspector.albumRequestRef.current += 1;
  const currentRequestId = fixture.input.inspector.albumRequestRef.current;
  albumTracks.set(currentTracks);
  selectedTrack.set(currentTracks[0] ?? null);
  rerender({ input: { ...fixture.input, inspector: {
    ...fixture.input.inspector, selectedAlbumId: selectedAlbumIdRef.current, albumTracks: currentTracks,
  } } });

  await act(async () => { finish(); await saving; });
  expect(load).not.toHaveBeenCalled();
  expect(fixture.input.inspector.albumRequestRef.current).toBe(currentRequestId);
  expect(albumTracks.value).toEqual(currentTracks);
  expect(selectedTrack.value).toEqual(currentTracks[0] ?? null);
  expect(fixture.explorerTracks.value[0].rating).toBe(4);
  expect(result.current.inlineTagRevisions[fixture.track.trackKey]).toBe(1);
  expect(result.current.inlineSavingKeys.size).toBe(0);
});

it("does not refresh a former album when navigation changes during tag-editor catalog sync", async () => {
  const fixture = options();
  const selectedAlbumIdRef = { current: fixture.track.albumId };
  Object.assign(fixture.input.inspector, { selectedAlbumId: fixture.track.albumId, selectedAlbumIdRef });
  let finish!: (value: boolean) => void;
  fixture.input.handleCatalogSync = vi.fn(() => new Promise<boolean>((resolve) => { finish = resolve; }));
  const load = vi.spyOn(library, "loadAlbumDetail");
  const { result } = renderHook(() => useTagMutations(fixture.input));
  let refreshing!: Promise<void>;
  act(() => { refreshing = result.current.refreshTagEditorCatalogViews({ status: "synced", pendingFolderCount: 0 }); });
  selectedAlbumIdRef.current = "preview-viva";
  fixture.input.inspector.albumRequestRef.current += 1;
  const currentRequestId = fixture.input.inspector.albumRequestRef.current;
  await act(async () => { finish(false); await refreshing; });
  expect(load).not.toHaveBeenCalled();
  expect(fixture.input.inspector.albumRequestRef.current).toBe(currentRequestId);
  expect(fixture.input.inspector.setAlbumTracks).not.toHaveBeenCalled();
});

it("uses the current album for a tag-editor callback captured before navigation", async () => {
  const fixture = options();
  const selectedAlbumIdRef = { current: fixture.track.albumId };
  Object.assign(fixture.input.inspector, { selectedAlbumId: fixture.track.albumId, selectedAlbumIdRef });
  const { result, rerender } = renderHook(({ input }) => useTagMutations(input), { initialProps: { input: fixture.input } });
  const refresh = result.current.refreshTagEditorCatalogViews;
  selectedAlbumIdRef.current = "preview-viva";
  rerender({ input: { ...fixture.input, inspector: { ...fixture.input.inspector, selectedAlbumId: "preview-viva" } } });
  let finish!: (value: boolean) => void;
  vi.mocked(fixture.input.handleCatalogSync).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  let refreshing!: Promise<void>;
  act(() => { refreshing = refresh({ status: "synced", pendingFolderCount: 0 }); });
  expect(fixture.input.explorer.pendingExplorerAlbumIdRef.current).toBe("preview-viva");
  await act(async () => { finish(true); await refreshing; });
});

it("refreshes verified rating metrics while the edited album remains selected", async () => {
  const fixture = options();
  const detail = await library.loadAlbumDetail(fixture.track.albumId!);
  const albumTracks = state(detail.tracks);
  Object.assign(fixture.input.inspector, {
    selectedAlbumId: detail.album.id, selectedAlbumIdRef: { current: detail.album.id },
    albumTracks: albumTracks.value, setAlbumTracks: albumTracks.set,
  });
  const desired = { ...tags.tagValuesForTrack(fixture.track), rating: 4 };
  const saved = { ...fixture.track, rating: 4, tagSyncState: null };
  vi.spyOn(tags, "updateTrackTags").mockResolvedValue({
    track: saved, tagState: { values: desired, syncState: null, canUndo: false },
    catalogSync: { status: "synced", pendingFolderCount: 0 },
  });
  const verifiedDetail = { ...detail, tracks: detail.tracks.map((track) => track.trackKey === saved.trackKey ? saved : track) };
  const load = vi.spyOn(library, "loadAlbumDetail").mockResolvedValue(verifiedDetail);
  const { result } = renderHook(() => useTagMutations(fixture.input));
  await act(async () => { await result.current.saveInlineTagChange(fixture.track, desired); });
  expect(load).toHaveBeenCalledWith(detail.album.id);
  expect(albumTracks.value.find((track) => track.trackKey === saved.trackKey)?.rating).toBe(4);
  expect(fixture.input.explorer.setExplorerAlbums).toHaveBeenCalled();
});
