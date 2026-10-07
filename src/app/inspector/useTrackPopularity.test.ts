import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import * as library from "../../library";
import { useTrackPopularity } from "./useTrackPopularity";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

const track: library.Track = {
  ...library.browserPreview.tracks[0],
  artist: "Various Artists", displayArtist: "10cc", title: "Lying Here With You", playCount: null,
};

it("does not look up popularity before a track is selected", () => {
  const load = vi.spyOn(library, "loadTrackPopularity");
  const { result } = renderHook(() => useTrackPopularity(null));
  expect(result.current).toBeNull();
  expect(load).not.toHaveBeenCalled();
});

it.each([0, 1, 999, 1000])("retrieves and retains a Last.fm play count of %s", async (playCount) => {
  const load = vi.spyOn(library, "loadTrackPopularity").mockResolvedValue({ playCount, listeners: 3 });
  const { result, rerender } = renderHook(({ value }) => useTrackPopularity(value), { initialProps: { value: track } });
  await waitFor(() => expect(result.current).toEqual({ listeners: 3, playCount }));
  expect(load).toHaveBeenCalledWith("10cc", "Lying Here With You");
  rerender({ value: { ...track, playCount: 50000 } });
  expect(result.current).toEqual({ listeners: 3, playCount });
  expect(load).toHaveBeenCalledTimes(1);
});

it("looks up a track again when returning to it and ignores a late previous selection", async () => {
  let finishOlder!: (value: library.TrackPopularity) => void;
  const load = vi.spyOn(library, "loadTrackPopularity")
    .mockImplementationOnce(() => new Promise((resolve) => { finishOlder = resolve; }))
    .mockResolvedValueOnce({ playCount: 15 })
    .mockResolvedValueOnce({ playCount: 42 });
  const { result, rerender } = renderHook(({ value }) => useTrackPopularity(value), { initialProps: { value: track } });
  rerender({ value: { ...track, trackKey: "other", title: "Other track" } });
  await waitFor(() => expect(result.current?.playCount).toBe(15));
  await act(async () => { finishOlder({ playCount: 900000 }); });
  expect(result.current?.playCount).toBe(15);
  rerender({ value: track });
  await waitFor(() => expect(result.current?.playCount).toBe(42));
  expect(load).toHaveBeenCalledTimes(3);
});

it("refreshes edited lookup metadata and keeps catalog evidence on a network failure", async () => {
  const load = vi.spyOn(library, "loadTrackPopularity")
    .mockResolvedValueOnce({ playCount: 10 })
    .mockRejectedValueOnce(new Error("Offline"));
  const { result, rerender } = renderHook(({ value }) => useTrackPopularity(value), { initialProps: { value: track } });
  await waitFor(() => expect(result.current?.playCount).toBe(10));
  rerender({ value: { ...track, title: "Corrected title", playCount: 7 } });
  await waitFor(() => expect(load).toHaveBeenLastCalledWith("10cc", "Corrected title"));
  expect(result.current?.playCount).toBe(7);
});

it("shows unknown when Last.fm has no track instead of reusing a stale count", async () => {
  vi.spyOn(library, "loadTrackPopularity").mockResolvedValue({ playCount: null });
  const { result } = renderHook(() => useTrackPopularity({ ...track, playCount: 5000 }));
  await waitFor(() => expect(result.current?.playCount).toBeNull());
});
