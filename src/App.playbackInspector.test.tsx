import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, beforeEach, expect, it, vi } from "vitest";
import App from "./App";
import * as library from "./library";
import * as history from "./history";
import * as playback from "./playback";

vi.mock("./components/WaveformTimeline", () => ({ WaveformTimeline: () => null }));

beforeAll(async () => {
  await Promise.all([import("./app/routes/LibraryRoute"), import("./app/routes/ChartsRoute")]);
});

let player: ReturnType<typeof playback.usePlayback>;
beforeEach(async () => {
  await playback.clearPlaybackQueue();
  await playback.changeShuffle(false);
  await playback.changeRepeatMode("off");
  const usePlayback = playback.usePlayback;
  vi.spyOn(playback, "usePlayback").mockImplementation(function useObservedPlayback() {
    player = usePlayback();
    return player;
  });
});

afterEach(async () => {
  cleanup();
  vi.restoreAllMocks();
  localStorage.clear();
  await playback.clearPlaybackQueue();
});

function inspector() {
  return within(document.querySelector(".inspector") as HTMLElement);
}

async function expectPlaying(track: library.Track) {
  await waitFor(() => {
    expect(document.querySelector(".now-playing__title strong")).toHaveTextContent(track.title);
    expect(inspector().getByRole("heading", { name: track.title })).toBeVisible();
  });
}

async function navigate(name: string) {
  fireEvent.click(within(screen.getByRole("navigation", { name: "Primary" })).getByRole("button", { name }));
  await act(async () => { await vi.dynamicImportSettled(); });
}

it("shows restored playback and its history even when the initial selection is another track", async () => {
  const track = library.browserPreview.tracks.find((candidate) => candidate.title === "Midnight City")!;
  const loadHistory = vi.spyOn(history, "loadTrackHistoryInsight").mockResolvedValue({
    sessions: 12, skips: 0, plays: 12, listenedSeconds: 90, lastListenedAtMs: null,
  });
  await playback.playTrackQueue([track], track.id);
  render(<App />);
  await screen.findByRole("region", { name: "Library overview" });
  await expectPlaying(track);
  await waitFor(() => expect(inspector().getByText("Your registered plays").nextElementSibling).toHaveTextContent("12"));
  expect(loadHistory).toHaveBeenLastCalledWith(track.trackKey);
});

it("keeps following playback across other album selections, delayed file refresh, next, previous and pause", async () => {
  const detail = await library.loadAlbumDetail("preview-viva");
  const queue = library.browserPreview.tracks.filter((track) => track.albumId !== detail.album.id).slice(0, 2);
  await playback.playTrackQueue(queue, queue[0].id);
  const load = library.loadAlbumDetail;
  let finishFiles!: (value: library.AlbumDetail) => void;
  vi.spyOn(library, "loadAlbumDetail").mockImplementation((id, options) => (
    id === detail.album.id && !options?.localOnly
      ? new Promise((resolve) => { finishFiles = resolve; })
      : load(id, options)
  ));
  render(<App />);
  await screen.findByRole("region", { name: "Library overview" });
  await navigate("Albums");
  fireEvent.click(await screen.findByRole("button", { name: /^Viva la Vida cover/ }));
  const album = within(await screen.findByRole("complementary", { name: "Viva la Vida album details" }));
  fireEvent.click(album.getByRole("row", { name: /Strawberry Swing/ }));
  fireEvent.click(inspector().getByRole("tab", { name: "Track" }));
  await expectPlaying(queue[0]);
  await waitFor(() => expect(finishFiles).toBeDefined());
  await act(async () => { finishFiles(detail); });
  await expectPlaying(queue[0]);

  // Publish an automatic queue transition through the same snapshot boundary as native events.
  await act(async () => { player.applySnapshot(await playback.nextTrack()); });
  await expectPlaying(queue[1]);
  expect(album.getByRole("row", { name: /Strawberry Swing/ })).toHaveAttribute("aria-selected", "true");
  fireEvent.click(screen.getByRole("button", { name: "Previous track" }));
  await expectPlaying(queue[0]);
  fireEvent.click(screen.getByRole("button", { name: "Next track" }));
  await expectPlaying(queue[1]);
  fireEvent.click(screen.getByRole("button", { name: "Pause" }));
  await waitFor(() => expect(player.state.status).toBe("paused"));
  fireEvent.click(album.getByRole("row", { name: /Strawberry Swing/ }));
  await expectPlaying(queue[1]);
});

it("keeps a selected chart song from overriding the playing track and resumes chart details after clearing playback", async () => {
  const queue = library.browserPreview.tracks.slice(0, 2);
  await playback.playTrackQueue(queue, queue[0].id);
  render(<App />);
  await screen.findByRole("region", { name: "Library overview" });
  await navigate("Charts");
  const table = await screen.findByRole("table", { name: "Official UK Singles Chart" });
  fireEvent.click(within(table).getByRole("row", { name: /Obsession/ }));
  fireEvent.click(inspector().getByRole("tab", { name: "Track" }));
  await expectPlaying(queue[0]);
  await act(async () => { player.applySnapshot(await playback.nextTrack()); });
  await expectPlaying(queue[1]);
  await act(async () => { player.applySnapshot(await playback.clearPlaybackQueue()); });
  expect(await inspector().findByRole("heading", { name: "Obsession" })).toBeVisible();
});

it("preserves the selected Tags track and draft when playback advances", async () => {
  const queue = library.browserPreview.tracks.filter((track) => track.albumId !== "preview-viva").slice(0, 2);
  await playback.playTrackQueue(queue, queue[0].id);
  render(<App />);
  await screen.findByRole("region", { name: "Library overview" });
  await navigate("Songs");
  fireEvent.click(await screen.findByRole("row", { name: /Strawberry Swing/ }));
  fireEvent.click(inspector().getByRole("tab", { name: "Tags" }));
  const title = await inspector().findByRole("textbox", { name: "Track title" });
  fireEvent.change(title, { target: { value: "Draft title" } });
  await act(async () => { player.applySnapshot(await playback.nextTrack()); });
  expect(inspector().getByRole("tab", { name: "Tags" })).toHaveAttribute("aria-selected", "true");
  expect(title).toHaveValue("Draft title");
  fireEvent.click(inspector().getByRole("tab", { name: "Track" }));
  await expectPlaying(queue[1]);
});
