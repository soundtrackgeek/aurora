import { act, cleanup, renderHook } from "@testing-library/react";
import { useRef, useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import * as library from "../../library";
import { loadViewPreferences } from "../../viewPreferences";
import { useInspectorSelection } from "./useInspectorSelection";

afterEach(() => { cleanup(); vi.restoreAllMocks(); localStorage.clear(); });

it("keeps a newer album selection when an older local detail request completes last", async () => {
  const albums = (await library.exploreAlbums({ pageSize: 2 })).items;
  const [older, newer] = await Promise.all(albums.map((album) => library.loadAlbumDetail(album.id)));
  let finishOlder!: (detail: library.AlbumDetail) => void;
  const pendingOlder = new Promise<library.AlbumDetail>((resolve) => { finishOlder = resolve; });
  const load = vi.spyOn(library, "loadAlbumDetail").mockImplementation(async (id) => (
    id === older.album.id ? pendingOlder : newer
  ));
  vi.spyOn(library, "loadAlbumPopularity").mockResolvedValue(newer.popularity);
  const { result } = renderHook(() => {
    const [selectedTrack, setSelectedTrack] = useState<library.Track | null>(null);
    const [, setExplorerAlbums] = useState(albums);
    const artistRequestRef = useRef(0);
    return {
      selectedTrack,
      ...useInspectorSelection({
        initialViewPreferences: loadViewPreferences(), selectedTrack, setSelectedTrack,
        artistRequestRef, setExplorerAlbums,
        playback: { state: { currentTrack: null }, play: async () => null },
        endGenreQueue: () => undefined, setSyncMessage: () => undefined,
      }),
    };
  });

  act(() => result.current.selectAlbum(older.album));
  await act(async () => { result.current.selectAlbum(newer.album); });
  expect(result.current.selectedAlbumId).toBe(newer.album.id);
  expect(result.current.albumDetailState).toBe("ready");
  expect(result.current.selectedTrack?.trackKey).toBe(newer.tracks[0].trackKey);

  await act(async () => { finishOlder(older); });
  expect(result.current.albumTracks.map((track) => track.trackKey)).toEqual(newer.tracks.map((track) => track.trackKey));
  expect(result.current.selectedTrack?.trackKey).toBe(newer.tracks[0].trackKey);
  expect(load.mock.calls.filter(([id]) => id === older.album.id)).toHaveLength(1);
});

it("does not reopen an album after selection was cleared while its detail loaded", async () => {
  const album = (await library.exploreAlbums({ pageSize: 1 })).items[0];
  const detail = await library.loadAlbumDetail(album.id);
  let finish!: (value: library.AlbumDetail) => void;
  vi.spyOn(library, "loadAlbumDetail").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  const { result } = renderHook(() => {
    const [selectedTrack, setSelectedTrack] = useState<library.Track | null>(null);
    const artistRequestRef = useRef(0);
    return useInspectorSelection({
      initialViewPreferences: loadViewPreferences(), selectedTrack, setSelectedTrack,
      artistRequestRef, setExplorerAlbums: () => undefined,
      playback: { state: { currentTrack: null }, play: async () => null },
      endGenreQueue: () => undefined, setSyncMessage: () => undefined,
    });
  });
  act(() => result.current.selectAlbum(album));
  act(() => result.current.selectAlbum(null));
  await act(async () => { finish(detail); });
  expect(result.current.selectedAlbumId).toBeNull();
  expect(result.current.albumTracks).toEqual([]);
  expect(result.current.inspectorView).toBe("track");
  expect(result.current.albumDetailState).toBe("ready");
});
