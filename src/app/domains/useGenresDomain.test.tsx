import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { loadGenreQueue, type GenreQueueMode } from "../../genres";
import { browserPreview, type Track } from "../../library";
import { useGenresDomain } from "./useGenresDomain";

vi.mock("../../genres", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../genres")>(),
  loadGenreQueue: vi.fn(),
}));

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  localStorage.clear();
});

it("does not play a genre queue that finishes after the genre selection changes", async () => {
  let resolveQueue!: (tracks: Track[]) => void;
  vi.mocked(loadGenreQueue).mockReturnValue(new Promise((resolve) => { resolveQueue = resolve; }));
  const play = vi.fn().mockResolvedValue(null);
  const { result } = renderHook(() => useGenresDomain({
    activeNav: "Genres",
    libraryReady: false,
    loadedPageRequestsRef: { current: new Map() },
    playback: { play, state: { queue: [], currentIndex: null } },
    appendPlayback: vi.fn().mockResolvedValue(null),
  }));

  act(() => result.current.setSelectedGenre("Rock"));
  let request!: Promise<void>;
  act(() => { request = result.current.startGenreQueue("radio" satisfies GenreQueueMode); });
  act(() => result.current.setSelectedGenre("Jazz"));
  await act(async () => {
    resolveQueue([browserPreview.tracks[0]]);
    await request;
  });

  expect(loadGenreQueue).toHaveBeenCalledWith({
    genre: "Rock", mode: "radio", limit: 100, excludeTrackKeys: [],
  });
  expect(play).not.toHaveBeenCalled();
  expect(result.current.selectedGenre).toBe("Jazz");
  expect(result.current.genreQueueBusy).toBeNull();
  expect(result.current.genreRadioSession).toBeNull();
});
