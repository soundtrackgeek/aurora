import { act, cleanup, renderHook } from "@testing-library/react";
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { useExplorerWorkspace } from "../explorer/useExplorerWorkspace";
import { explorerRequestKey } from "../explorer/explorerQueries";
import { useWorkspaceRestoration } from "./useWorkspaceRestoration";
import { useNavigationState, useWorkspaceNavigation, type NavigationSelectionPort } from "./useWorkspaceNavigation";
import { defaultViewPreferences } from "../../viewPreferences";
import type { SidebarDestination } from "../../components/navigation/SidebarNavigation";
import { defaultExplorerFilters } from "../../viewPreferences";

function selectionPort(): NavigationSelectionPort {
  return {
    selectedAlbumId: "album", setSelectedAlbumId: vi.fn(), albumTracks: [], setAlbumTracks: vi.fn(),
    albumTracksTruncated: false, setAlbumTracksTruncated: vi.fn(), albumDetailState: "ready", setAlbumDetailState: vi.fn(),
    selectedArtistId: null, setSelectedArtistId: vi.fn(), selectedTrack: null, setSelectedTrack: vi.fn(),
    inspectorView: "album", setInspectorView: vi.fn(), tagSelectionKind: "album", setTagSelectionKind: vi.fn(),
    inspectorArtistName: null, setInspectorArtistName: vi.fn(), artistDetail: null, setArtistDetail: vi.fn(),
    artistIntelligence: null, setArtistIntelligence: vi.fn(), artistWorldState: "loading", setArtistWorldState: vi.fn(),
    artistWorldError: null, setArtistWorldError: vi.fn(), albumRequestRef: { current: 0 }, artistRequestRef: { current: 0 },
  };
}

function useNavigation(selection: NavigationSelectionPort) {
  const [activeNav, setActiveNavState] = useState<SidebarDestination>("Songs");
  const explorer = useExplorerWorkspace(defaultViewPreferences);
  const workspace = useWorkspaceRestoration();
  const navigation = useNavigationState();
  const actions = useWorkspaceNavigation({
    navigation, workspace, explorer, selection, activeNav, setActiveNavState,
    setHistoryLoadingMore: vi.fn(), setReviewLoadingMore: vi.fn(), changeExplorerView: vi.fn(), expandLibraryNavigation: vi.fn()
  });
  return { activeNav, explorer, workspace, navigation, ...actions };
}

afterEach(() => { cleanup(); vi.restoreAllMocks(); localStorage.clear(); });

it("opens a pinned view without letting retained explorer filters override it",()=>{
  const selection=selectionPort();
  const {result}=renderHook(()=>useNavigation(selection));
  act(()=>result.current.explorer.setExplorerFilters(f=>({...f,query:"old query"})));
  act(()=>result.current.navigate("Charts"));
  act(()=>result.current.openSavedExplorerView("albums",{...defaultExplorerFilters,query:"genre:synthwave",sort:"yearDesc"}));
  expect(result.current.activeNav).toBe("Albums");
  expect(result.current.explorer.explorerView).toBe("albums");
  expect(result.current.explorer.explorerFilters.query).toBe("genre:synthwave");
  expect(result.current.explorer.explorerFilters.sort).toBe("yearDesc");
  expect(result.current.explorer.explorerCursor).toBeNull();
  expect(selection.setSelectedAlbumId).toHaveBeenLastCalledWith(null);
});

it("restores the retained view and requests refresh when its catalog revision changed", () => {
  const selection = selectionPort();
  const { result } = renderHook(() => useNavigation(selection));
  const container = document.createElement("div");
  container.scrollTop = 275;
  act(() => {
    result.current.workspace.mainScrollRef.current = container;
    result.current.explorer.setExplorerLoadState("ready");
    result.current.explorer.setExplorerFilters((filters) => ({ ...filters, query: "remember me" }));
  });
  const key = explorerRequestKey("tracks", result.current.explorer.explorerFilters, 0);
  act(() => {
    result.current.explorer.loadedExplorerRequestKeyRef.current = key;
    result.current.explorer.loadedExplorerViewKeyRef.current = key;
    result.current.navigate("Charts");
  });
  expect(result.current.activeNav).toBe("Charts");
  expect(result.current.navigation.navigationHistory).toHaveLength(1);
  expect(selection.albumRequestRef.current).toBe(1);
  expect(selection.artistRequestRef.current).toBe(1);
  act(() => {
    result.current.explorer.setExplorerFilters((filters) => ({ ...filters, query: "changed elsewhere" }));
    result.current.explorer.setExplorerReloadToken(1);
  });
  act(() => result.current.goBack());
  expect(result.current.activeNav).toBe("Songs");
  expect(result.current.explorer.explorerFilters.query).toBe("remember me");
  expect(result.current.workspace.scrollPositionByDestinationRef.current.Songs).toBe(275);
  expect(result.current.explorer.preserveExplorerOnReloadRef.current).toBe(true);
  expect(result.current.explorer.loadedExplorerRequestKeyRef.current).toBe(key);
  expect(selection.setSelectedAlbumId).toHaveBeenLastCalledWith("album");
  expect(selection.setInspectorView).toHaveBeenLastCalledWith("album");
  expect(result.current.navigation.navigationHistory).toHaveLength(0);
});

it("reloads an album detail that was still in flight when its view was left", () => {
  const selection = selectionPort();
  selection.albumDetailState = "loading";
  const { result } = renderHook(() => useNavigation(selection));
  act(() => result.current.explorer.setExplorerLoadState("ready"));
  act(() => {
    result.current.explorer.loadedExplorerRequestKeyRef.current = "previous request";
    result.current.navigate("Charts");
  });
  act(() => result.current.goBack());
  expect(result.current.explorer.loadedExplorerRequestKeyRef.current).toBeNull();
  expect(result.current.explorer.preserveExplorerOnReloadRef.current).toBe(true);
  expect(selection.setAlbumDetailState).toHaveBeenLastCalledWith("ready");
});
