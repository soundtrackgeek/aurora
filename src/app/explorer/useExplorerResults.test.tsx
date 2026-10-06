import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useExplorerWorkspace } from "./useExplorerWorkspace";
import { useExplorerResults, type ExplorerSelectionPort } from "./useExplorerResults";
import * as queries from "./explorerQueries";
import { useWorkspaceRestoration } from "../navigation/useWorkspaceRestoration";
import { defaultViewPreferences, type ViewPreferences } from "../../viewPreferences";
import * as library from "../../library";
import type { Track } from "../../library";
import { useRef, useState } from "react";
import { useInspectorSelection } from "../inspector/useInspectorSelection";

const track: Track = {
  id: "first", trackKey: "first", albumId: "album", title: "First", artist: "Artist", album: "Album",
  releaseYear: 2000, rating: null, loved: false, loveState: "neutral", tagSyncState: null,
  canUndoTagEdit: false, durationSeconds: 120, genre: null, playCount: null,
};

function selectionPort(): ExplorerSelectionPort {
  return {
    selectedAlbumIdRef: { current: null }, selectedTrackRef: { current: null }, albumRequestRef: { current: 0 },
    setSelectedAlbumId: vi.fn(), setSelectedTrack: vi.fn(), setAlbumTracks: vi.fn(),
    setAlbumTracksTruncated: vi.fn(), setAlbumDetailState: vi.fn(), setAlbumFileRefreshRequest: vi.fn(),
  };
}

function page(title: string, hasMore = false): queries.ExplorerResult {
  return {
    tracks: [{ ...track, title }], albums: [], artists: [], totalCount: hasMore ? 2 : 1,
    nextCursor: hasMore ? { value: title, id: title } : null
  };
}

function useResults(selection: ExplorerSelectionPort, preferences: ViewPreferences = defaultViewPreferences) {
  const explorer = useExplorerWorkspace(preferences);
  const workspace = useWorkspaceRestoration();
  const results = useExplorerResults({
    explorer, workspace, selection, activeNav: preferences.activeNav, artistPageName: null,
    libraryReady: true, initialSelectedAlbumId: null
  });
  return { ...explorer, ...results };
}

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.useRealTimers(); localStorage.clear(); });

it("rejects a late response from the search replaced during debounce", async () => {
  vi.useFakeTimers();
  let resolveFirst!: (result: queries.ExplorerResult) => void;
  const first = new Promise<queries.ExplorerResult>((resolve) => { resolveFirst = resolve; });
  vi.spyOn(queries, "loadExplorerPage").mockImplementation((_view, filters) => filters.query === ""
    ? first : Promise.resolve(page("Replacement")));
  const selection = selectionPort();
  const { result } = renderHook(() => useResults(selection));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  act(() => result.current.setExplorerFilters((filters) => ({ ...filters, query: "replacement" })));
  await act(async () => { await vi.advanceTimersByTimeAsync(2_000); });
  expect(result.current.explorerTracks.map((item) => item.title)).toEqual(["Replacement"]);
  await act(async () => { resolveFirst(page("Obsolete")); await Promise.resolve(); });
  expect(result.current.explorerTracks.map((item) => item.title)).toEqual(["Replacement"]);
  expect(result.current.explorerLoadState).toBe("ready");
});

it("does not paginate with the old cursor while a new query is waiting", async () => {
  vi.useFakeTimers();
  const load = vi.spyOn(queries, "loadExplorerPage").mockResolvedValue(page("Loaded", true));
  const selection = selectionPort();
  const { result } = renderHook(() => useResults(selection));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  expect(result.current.explorerCursor).not.toBeNull();
  load.mockClear();
  act(() => result.current.setExplorerFilters((filters) => ({ ...filters, query: "replacement" })));
  await act(async () => { await result.current.loadMoreExplorerResults(); });
  expect(load).not.toHaveBeenCalled();
  await act(async () => { await vi.advanceTimersByTimeAsync(2_000); });
  expect(load).toHaveBeenCalledWith("tracks", expect.objectContaining({ query: "replacement" }), undefined, true);
  expect(result.current.explorerTracks).toHaveLength(1);
});

it.each(["another album", "closed details"])("preserves navigation to %s during a background catalog refresh", async (destination) => {
  const albums = (await library.exploreAlbums({ pageSize: 2 })).items;
  const older = await library.loadAlbumDetail(albums[0].id);
  vi.useFakeTimers();
  const refreshed: queries.ExplorerResult = { tracks: [], artists: [], albums, totalCount: 2, nextCursor: null };
  const loadPage = vi.spyOn(queries, "loadExplorerPage").mockResolvedValue(refreshed);
  const loadDetail = vi.spyOn(library, "loadAlbumDetail").mockResolvedValue(older);
  const selection = selectionPort();
  const { result } = renderHook(() => useResults(selection, {
    ...defaultViewPreferences, activeNav: "Albums", explorerView: "albums",
  }));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  selection.selectedAlbumIdRef.current = older.album.id;
  selection.selectedTrackRef.current = older.tracks[0];
  result.current.pendingExplorerAlbumIdRef.current = older.album.id;
  result.current.preserveExplorerOnReloadRef.current = true;
  let finish!: (value: queries.ExplorerResult) => void;
  loadPage.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  act(() => result.current.setExplorerReloadToken((value) => value + 1));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  selection.selectedAlbumIdRef.current = destination === "another album" ? albums[1].id : null;
  selection.albumRequestRef.current += 1;
  const currentRequestId = selection.albumRequestRef.current;
  await act(async () => { finish(refreshed); });
  expect(loadDetail).not.toHaveBeenCalled();
  expect(selection.setSelectedAlbumId).not.toHaveBeenCalledWith(older.album.id);
  expect(selection.setAlbumTracks).not.toHaveBeenCalledWith(older.tracks);
  expect(selection.albumRequestRef.current).toBe(currentRequestId);
  expect(result.current.explorerAlbums).toEqual(albums);
  expect(result.current.explorerLoadState).toBe("ready");
  expect(result.current.pendingExplorerAlbumIdRef.current).toBeNull();
});

it.each(["another album", "closed details"])("ignores an obsolete handoff when %s was chosen before the refresh starts", async (destination) => {
  const albums = (await library.exploreAlbums({ pageSize: 2 })).items;
  const details = await Promise.all(albums.map((album) => library.loadAlbumDetail(album.id)));
  vi.useFakeTimers();
  vi.spyOn(queries, "loadExplorerPage").mockResolvedValue({ tracks: [], artists: [], albums, totalCount: 2, nextCursor: null });
  const loadDetail = vi.spyOn(library, "loadAlbumDetail").mockImplementation(async (id) => details.find((detail) => detail.album.id === id)!);
  const selection = selectionPort();
  const { result } = renderHook(() => useResults(selection, {
    ...defaultViewPreferences, activeNav: "Albums", explorerView: "albums",
  }));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  result.current.pendingExplorerAlbumIdRef.current = albums[0].id;
  selection.selectedAlbumIdRef.current = destination === "another album" ? albums[1].id : null;
  selection.albumRequestRef.current += 1;
  result.current.preserveExplorerOnReloadRef.current = true;
  act(() => result.current.setExplorerReloadToken((value) => value + 1));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  expect(loadDetail.mock.calls.some(([id]) => id === albums[0].id)).toBe(false);
  expect(selection.setSelectedAlbumId).not.toHaveBeenCalledWith(albums[0].id);
  expect(result.current.pendingExplorerAlbumIdRef.current).toBeNull();
});

it("lets the clicked album finish opening before a slower background catalog refresh", async () => {
  const albums = (await library.exploreAlbums({ pageSize: 2 })).items;
  const detail = await library.loadAlbumDetail(albums[1].id);
  const refreshed = { tracks: [], artists: [], albums, totalCount: 2, nextCursor: null };
  vi.useFakeTimers();
  const loadPage = vi.spyOn(queries, "loadExplorerPage").mockResolvedValue(refreshed);
  vi.spyOn(library, "loadAlbumPopularity").mockResolvedValue(detail.popularity);
  let finishOpening!: (value: library.AlbumDetail) => void;
  const loadDetail = vi.spyOn(library, "loadAlbumDetail").mockResolvedValue(detail)
    .mockImplementationOnce(() => new Promise((resolve) => { finishOpening = resolve; }));
  const { result } = renderHook(() => {
    const preferences = { ...defaultViewPreferences, activeNav: "Albums" as const, explorerView: "albums" as const };
    const explorer = useExplorerWorkspace(preferences);
    const workspace = useWorkspaceRestoration();
    const [selectedTrack, setSelectedTrack] = useState<Track | null>(null);
    const artistRequestRef = useRef(0);
    const selection = useInspectorSelection({
      initialViewPreferences: preferences, selectedTrack, setSelectedTrack, artistRequestRef,
      setExplorerAlbums: explorer.setExplorerAlbums,
      playback: { state: { currentTrack: null }, play: async () => null },
      endGenreQueue: () => undefined, setSyncMessage: () => undefined,
    });
    useExplorerResults({ explorer, workspace, selection: { ...selection, setSelectedTrack },
      activeNav: "Albums", artistPageName: null, libraryReady: true, initialSelectedAlbumId: null });
    return { ...explorer, ...selection, selectedTrack };
  });
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  act(() => result.current.selectAlbum(albums[1]));
  const openingRequest = result.current.albumRequestRef.current;
  let finishRefresh!: (value: queries.ExplorerResult) => void;
  loadPage.mockImplementationOnce(() => new Promise((resolve) => { finishRefresh = resolve; }));
  result.current.preserveExplorerOnReloadRef.current = true;
  act(() => result.current.setExplorerReloadToken((value) => value + 1));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  expect(result.current.albumRequestRef.current).toBe(openingRequest);
  await act(async () => { finishOpening(detail); });
  expect(result.current.albumDetailState).toBe("ready");
  expect(result.current.albumTracks).toEqual(library.applyAlbumPopularity(detail.tracks, detail.popularity));
  expect(result.current.selectedAlbumId).toBe(detail.album.id);
  // Track selection made after refresh start must also survive its late detail.
  act(() => result.current.selectTrack(detail.tracks[1]));
  await act(async () => { finishRefresh(refreshed); });
  expect(result.current.selectedTrack?.trackKey).toBe(detail.tracks[1].trackKey);
  expect(result.current.selectedAlbumId).toBe(detail.album.id);
  expect(result.current.albumRequestRef.current).toBe(openingRequest);
  expect(loadDetail).toHaveBeenCalledWith(detail.album.id, { localOnly: true });
});

it("still opens an explicit album handoff outside the first result page", async () => {
  const albums = (await library.exploreAlbums({ pageSize: 2 })).items;
  const detail = await library.loadAlbumDetail(albums[1].id);
  vi.useFakeTimers();
  vi.spyOn(queries, "loadExplorerPage").mockImplementation(async (_view, _filters, _cursor, localOnly) => ({
    tracks: [], artists: [], albums: localOnly ? [albums[0]] : albums, totalCount: 2, nextCursor: null,
  }));
  const loadDetail = vi.spyOn(library, "loadAlbumDetail").mockResolvedValue(detail);
  const selection = selectionPort();
  const { result } = renderHook(() => useResults(selection, {
    ...defaultViewPreferences, activeNav: "Albums", explorerView: "albums",
  }));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  selection.selectedAlbumIdRef.current = detail.album.id;
  result.current.pendingExplorerAlbumIdRef.current = detail.album.id;
  act(() => result.current.setExplorerReloadToken((value) => value + 1));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  expect(loadDetail).toHaveBeenCalledWith(detail.album.id, { localOnly: true });
  expect(selection.setSelectedAlbumId).toHaveBeenCalledWith(detail.album.id);
  expect(selection.setAlbumTracks).toHaveBeenCalledWith(library.applyAlbumPopularity(detail.tracks, detail.popularity));
  expect(result.current.explorerAlbums.some((album) => album.id === detail.album.id)).toBe(true);
});
