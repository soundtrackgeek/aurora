import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { loadChartEntryTrack } from "./charts";
import { loadAlbumDetail, type Track } from "./library";
import { loadMusicLibraryPlaylist, type SavedPlaylistDetail } from "./playlists";
import { handlePlaybackLink, parsePlaybackLink, useMusicLibraryPlaybackLinks } from "./musicLibraryPlayback";

vi.mock("@tauri-apps/plugin-deep-link", () => ({ getCurrent: vi.fn(), onOpenUrl: vi.fn() }));
vi.mock("./charts", () => ({ loadChartEntryTrack: vi.fn() }));
vi.mock("./library", () => ({ isTauriRuntime: () => true, loadAlbumDetail: vi.fn() }));
vi.mock("./playlists", () => ({ loadMusicLibraryPlaylist: vi.fn() }));

const albumId = "mb:album/ø";
const song = { id: "42", trackKey: "track", albumId, loveState: "neutral" } as Track;

beforeEach(() => vi.resetAllMocks());

it("parses catalog IDs and rejects unrecognized or malformed links", () => {
  expect(parsePlaybackLink("aurora://play/album/mb%3Aalbum%2F%C3%B8")).toEqual({ action: "play", kind: "album", albumId });
  expect(parsePlaybackLink("aurora://queue/track/42?album=mb%3Aalbum%2F%C3%B8")).toEqual({ action: "queue", kind: "track", albumId, trackId: "42" });
  for (const url of ["https://play/album/a", "aurora://play/track/42", "aurora://play/playlist/0", "aurora://play/album/%00", "aurora://play/album/a/b", "aurora://other/album/a"]) {
    expect(parsePlaybackLink(url)).toBeNull();
  }
});

it("rejects a track ID that now belongs to another album before touching playback", async () => {
  vi.mocked(loadChartEntryTrack).mockResolvedValue({ ...song, albumId: "changed" });
  const handlers = { play: vi.fn(), queue: vi.fn() };
  await expect(handlePlaybackLink({ action: "play", kind: "track", albumId, trackId: "42" }, handlers)).rejects.toThrow("changed in the catalog");
  expect(handlers.play).not.toHaveBeenCalled();
  expect(handlers.queue).not.toHaveBeenCalled();
});

it("plays a saved playlist with its paging context and queues all pages in one call", async () => {
  const handlers = { play: vi.fn().mockResolvedValue(true), queue: vi.fn().mockResolvedValue(true) };
  const another = { ...song, id: "43" };
  const first = { tracks: [song], nextCursor: 4, revision: "rev" } as SavedPlaylistDetail;
  const second = { tracks: [another], nextCursor: null } as SavedPlaylistDetail;
  vi.mocked(loadMusicLibraryPlaylist).mockResolvedValueOnce(first);
  await handlePlaybackLink({ action: "play", kind: "playlist", playlistId: 9 }, handlers);
  expect(handlers.play).toHaveBeenCalledWith([song], { id: 9, cursor: 4, revision: "rev" });

  vi.mocked(loadMusicLibraryPlaylist).mockResolvedValueOnce(first).mockResolvedValueOnce(second);
  await handlePlaybackLink({ action: "queue", kind: "playlist", playlistId: 9 }, handlers);
  expect(loadMusicLibraryPlaylist).toHaveBeenLastCalledWith(9, 4, "rev");
  expect(handlers.queue).toHaveBeenCalledExactlyOnceWith([song, another]);
});

it("handles a cold or live playback link and deduplicates delivery of the same click", async () => {
  const first = "aurora://play/album/mb%3Aalbum%2F%C3%B8?request=1";
  const next = "aurora://queue/track/42?album=mb%3Aalbum%2F%C3%B8&request=2";
  let receive!: (urls: string[]) => void;
  const release = vi.fn();
  vi.mocked(onOpenUrl).mockImplementation(async handler => { receive = handler; return release; });
  vi.mocked(getCurrent).mockResolvedValue([first]);
  vi.mocked(loadAlbumDetail).mockResolvedValue({ tracks: [song] } as Awaited<ReturnType<typeof loadAlbumDetail>>);
  vi.mocked(loadChartEntryTrack).mockResolvedValue(song);
  const play = vi.fn().mockResolvedValue(true);
  const queue = vi.fn().mockResolvedValue(true);
  const { unmount } = renderHook(() => useMusicLibraryPlaybackLinks({ play, queue }, vi.fn()));
  await waitFor(() => expect(play).toHaveBeenCalledExactlyOnceWith([song], undefined));
  receive([first, next]);
  await waitFor(() => expect(queue).toHaveBeenCalledExactlyOnceWith([song]));
  expect(play).toHaveBeenCalledTimes(1);
  unmount();
  expect(release).toHaveBeenCalledOnce();
});
