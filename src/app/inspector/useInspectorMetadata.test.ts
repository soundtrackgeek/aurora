import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import * as charts from "../../charts";
import * as history from "../../history";
import { browserPreview } from "../../library";
import { useInspectorMetadata, type InspectorMetadataOptions } from "./useInspectorMetadata";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

function options(): InspectorMetadataOptions {
  return {
    selectedTrack: browserPreview.tracks[0], inspectorTrack: browserPreview.tracks[1],
    setSelectedTrack: vi.fn(), selectedAlbumId: null, albumTracks: [], explorerView: "tracks",
    explorerTracks: [], explorerAlbums: [], selectedYearAlbum: null, snapshot: null,
    yearAlbumTracks: [], ratingAlbumTracks: [], publisherAlbumTracks: [], genreDetail: null,
  };
}

it("loads history for playback independently of selection and rejects a late previous track response", async () => {
  const input = options();
  let finishOlder!: (value: history.TrackHistoryInsight) => void;
  const load = vi.spyOn(history, "loadTrackHistoryInsight")
    .mockImplementationOnce(() => new Promise((resolve) => { finishOlder = resolve; }))
    .mockResolvedValueOnce({ sessions: 9, skips: 0, plays: 9, listenedSeconds: 60, lastListenedAtMs: null });
  vi.spyOn(charts, "loadCatalogChartRankings").mockResolvedValue({ tracks: {}, albums: {} });
  const { result, rerender } = renderHook(useInspectorMetadata, { initialProps: input });
  expect(load).toHaveBeenCalledWith(input.inspectorTrack!.trackKey);
  const newer = { ...input, inspectorTrack: browserPreview.tracks[2] };
  rerender(newer);
  await waitFor(() => expect(result.current.trackHistory).toEqual({
    trackKey: newer.inspectorTrack.trackKey, value: { sessions: 9, skips: 0, plays: 9, listenedSeconds: 60, lastListenedAtMs: null },
  }));
  await act(async () => { finishOlder({ sessions: 99, skips: 0, plays: 99, listenedSeconds: 999, lastListenedAtMs: null }); });
  expect(result.current.trackHistory?.value.plays).toBe(9);
  expect(load).toHaveBeenLastCalledWith(newer.inspectorTrack.trackKey);
});

it("includes playback outside a full visible page in bounded chart rankings without reloading on position snapshots", async () => {
  const input = options();
  input.explorerTracks = Array.from({ length: 100 }, (_, index) => ({
    ...browserPreview.tracks[0], id: `visible-${index}`, trackKey: `visible-${index}`,
  }));
  const load = vi.spyOn(charts, "loadCatalogChartRankings").mockResolvedValue({ tracks: {}, albums: {} });
  vi.spyOn(history, "loadTrackHistoryInsight").mockResolvedValue({ sessions: 0, skips: 0, plays: 0, listenedSeconds: 0, lastListenedAtMs: null });
  const { rerender } = renderHook(useInspectorMetadata, { initialProps: input });
  expect(load).toHaveBeenCalledWith({
    trackIds: [input.inspectorTrack!.id, input.selectedTrack!.id, ...input.explorerTracks.slice(0, 98).map((track) => track.id)],
    albumIds: [],
  });
  await act(async () => { rerender({ ...input, inspectorTrack: { ...input.inspectorTrack! } }); });
  expect(load).toHaveBeenCalledTimes(1);
});
