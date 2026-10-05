import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useExplorerWorkspace } from "./useExplorerWorkspace";
import { useExplorerResults, type ExplorerSelectionPort } from "./useExplorerResults";
import * as queries from "./explorerQueries";
import { useWorkspaceRestoration } from "../navigation/useWorkspaceRestoration";
import { defaultViewPreferences, type ViewPreferences } from "../../viewPreferences";
import * as library from "../../library";
import type { Track } from "../../library";

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
