import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { SidebarDestination } from "../../components/navigation/SidebarNavigation";
import { loadHistoryPage, type HistoryPage } from "../../history";
import { useHistoryDomain } from "./useHistoryDomain";

vi.mock("../../history", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../history")>(),
  loadHistoryPage: vi.fn(),
}));

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.resetAllMocks();
});

function page(syncMessage: string): HistoryPage {
  return {
    items: [], summary: { sessions: 0, plays: 0, skips: 0, uniqueTracks: 0, listenedSeconds: 0 },
    topTracks: [], devices: [], nextCursor: null, playThresholdSeconds: 30,
    syncState: "synced", syncMessage,
  };
}

function options() {
  return {
    libraryReady: true,
    loadedPageRequestsRef: { current: new Map<string, string>() },
    playback: { play: vi.fn().mockResolvedValue(null) },
    endGenreQueue: vi.fn(),
    selectTrack: vi.fn(),
  };
}

it("ignores a superseded history search without replacing the current page", async () => {
  vi.useFakeTimers();
  let resolveOld!: (value: HistoryPage) => void;
  vi.mocked(loadHistoryPage)
    .mockReturnValueOnce(new Promise((resolve) => { resolveOld = resolve; }))
    .mockResolvedValueOnce(page("current search"));
  const inputs = options();
  const { result } = renderHook(() => useHistoryDomain({ ...inputs, activeNav: "History" }));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  act(() => result.current.setHistorySearch("New Order"));
  await act(async () => { await vi.advanceTimersByTimeAsync(160); });
  expect(result.current.historyPage?.syncMessage).toBe("current search");

  await act(async () => { resolveOld(page("outdated search")); });

  expect(result.current.historyPage?.syncMessage).toBe("current search");
  expect(loadHistoryPage).toHaveBeenLastCalledWith({
    pageSize: 50, search: "New Order", outcome: "all", deviceId: undefined, startedAfterMs: undefined,
  });
});

it("retains a loaded history destination until its reload token is invalidated", async () => {
  vi.useFakeTimers();
  vi.mocked(loadHistoryPage).mockResolvedValue(page("retained page"));
  const inputs = options();
  const { result, rerender } = renderHook(
    ({ activeNav }: { activeNav: SidebarDestination; }) => useHistoryDomain({ ...inputs, activeNav }),
    { initialProps: { activeNav: "History" } },
  );
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  rerender({ activeNav: "Albums" });
  rerender({ activeNav: "History" });
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  expect(loadHistoryPage).toHaveBeenCalledTimes(1);
  expect(result.current.historyPage?.syncMessage).toBe("retained page");

  act(() => result.current.setHistoryReloadToken((token) => token + 1));
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  expect(loadHistoryPage).toHaveBeenCalledTimes(2);
});
