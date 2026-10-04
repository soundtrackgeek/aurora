import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import * as library from "../../library";
import { rebindPlaybackCatalog, type PlaybackCatalogRebind } from "../../playback";
import { useCatalogRevision } from "./useCatalogRevision";

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.useRealTimers(); });

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((finish) => { resolve = finish; });
  return { promise, resolve };
}

async function options() {
  const rebound = await rebindPlaybackCatalog();
  return {
    libraryReady: false,
    latestTagProjectionTokenRef: { current: 0 },
    rebindPlaybackCatalog: vi.fn(async (): Promise<PlaybackCatalogRebind | null> => ({
      ...rebound, catalogRevision: "next",
    })),
    onCatalogRefresh: vi.fn(),
  };
}

it("coalesces overlapping requests and applies only a consistent snapshot", async () => {
  const input = await options();
  const revision = deferred<string>();
  const loadRevision = vi.spyOn(library, "loadCatalogRevision")
    .mockReturnValueOnce(revision.promise).mockResolvedValue("next");
  vi.spyOn(library, "loadLibrarySnapshot").mockResolvedValue({ ...library.browserPreview, catalogRevision: "next" });
  const { result } = renderHook(() => useCatalogRevision(input));
  result.current.catalogRevisionRef.current = "previous";
  const first = result.current.refreshCatalogIfChanged();
  const second = result.current.refreshCatalogIfChanged();
  expect(first).toBe(second);
  await act(async () => { revision.resolve("next"); expect(await first).toBe(true); });
  expect(input.rebindPlaybackCatalog).toHaveBeenCalledTimes(1);
  expect(input.onCatalogRefresh).toHaveBeenCalledTimes(1);
  expect(loadRevision).toHaveBeenCalledTimes(2);
  expect(result.current.catalogRevisionRef.current).toBe("next");
});

it("retries a snapshot overtaken by a local tag edit without projecting stale tags", async () => {
  const input = await options();
  vi.spyOn(library, "loadCatalogRevision").mockResolvedValue("next");
  const staleSnapshot = deferred<library.LibrarySnapshot>();
  const loadSnapshot = vi.spyOn(library, "loadLibrarySnapshot")
    .mockReturnValueOnce(staleSnapshot.promise)
    .mockResolvedValue({ ...library.browserPreview, catalogRevision: "next" });
  const { result } = renderHook(() => useCatalogRevision(input));
  result.current.catalogRevisionRef.current = "previous";
  const refresh = result.current.refreshCatalogIfChanged();
  await act(async () => { await Promise.resolve(); });
  expect(loadSnapshot).toHaveBeenCalledTimes(1);
  input.latestTagProjectionTokenRef.current = 10;
  await act(async () => {
    staleSnapshot.resolve({ ...library.browserPreview, catalogRevision: "next" });
    expect(await refresh).toBe(true);
  });
  expect(loadSnapshot).toHaveBeenCalledTimes(2);
  expect(input.onCatalogRefresh).toHaveBeenCalledTimes(1);
});

it("rejects an inconsistent catalog after the bounded retry and leaves the old revision intact", async () => {
  const input = await options();
  vi.spyOn(library, "loadCatalogRevision").mockResolvedValue("next");
  vi.spyOn(library, "loadLibrarySnapshot").mockResolvedValue({ ...library.browserPreview, catalogRevision: "newer" });
  const { result } = renderHook(() => useCatalogRevision(input));
  result.current.catalogRevisionRef.current = "previous";
  await expect(result.current.refreshCatalogIfChanged()).rejects.toThrow("catalog changed again");
  expect(input.rebindPlaybackCatalog).toHaveBeenCalledTimes(2);
  expect(input.onCatalogRefresh).not.toHaveBeenCalled();
  expect(result.current.catalogRevisionRef.current).toBe("previous");
});

it("cancels a pending poll and removes its timer and focus listener on unmount", async () => {
  vi.useFakeTimers();
  const input = { ...await options(), libraryReady: true };
  const revision = deferred<string>();
  const loadRevision = vi.spyOn(library, "loadCatalogRevision").mockReturnValue(revision.promise);
  const { result, unmount } = renderHook(() => useCatalogRevision(input));
  result.current.catalogRevisionRef.current = "previous";
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
  expect(loadRevision).toHaveBeenCalledTimes(1);
  unmount();
  await act(async () => { revision.resolve("next"); await vi.advanceTimersByTimeAsync(10_000); });
  window.dispatchEvent(new Event("focus"));
  expect(loadRevision).toHaveBeenCalledTimes(1);
  expect(input.rebindPlaybackCatalog).not.toHaveBeenCalled();
  expect(input.onCatalogRefresh).not.toHaveBeenCalled();
});
