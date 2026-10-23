import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { browserPreview } from "../library";
import { sonicWorlds, type WorldResponse } from "../sonicWorlds";
import { SonicWorldsPanel } from "./SonicWorldsPanel";
import { PlaylistAuthoringContext } from "./playlists/playlistAuthoringContext";

vi.mock("../sonicWorlds", () => ({ sonicWorlds: vi.fn() }));
const seed = browserPreview.tracks[0];
const candidates = browserPreview.tracks.slice(1, 4);
const controls = { onPause: vi.fn(), playing: true, playbackError: null };
const response: WorldResponse = { version: "sonic-worlds-v1", seedReady: true, seedValue: "1980s", maxDistance: 0.25, searchLimited: true,
  matches: candidates.map(track => ({ track, value: "2010s", distance: 0.123 })) };
beforeEach(() => { HTMLDialogElement.prototype.showModal = vi.fn(function (this: HTMLDialogElement) { this.open = true; }); });
afterEach(() => { cleanup(); vi.resetAllMocks(); });

it("conceals identities until reveal, plays one real track, and saves the reviewed order", async () => {
  vi.mocked(sonicWorlds).mockResolvedValue(response);
  const play = vi.fn(), save = vi.fn();
  render(<PlaylistAuthoringContext value={save}><SonicWorldsPanel {...controls} track={seed} onPlay={play} onClose={vi.fn()} /></PlaylistAuthoringContext>);
  fireEvent.click(screen.getByText("Find three sounds"));
  await screen.findByText("Candidate 1");
  for (const track of candidates) expect(screen.queryByText(track.title)).not.toBeInTheDocument();
  expect(screen.queryByText("Save discoveries")).not.toBeInTheDocument();
  expect(screen.getByText(/system media controls/)).toBeInTheDocument();
  fireEvent.click(screen.getByLabelText("Play candidate 2"));
  expect(play).toHaveBeenCalledWith(candidates[1], [candidates[1]]);
  fireEvent.click(screen.getByText("Pause playback"));
  expect(controls.onPause).toHaveBeenCalledOnce();
  expect(screen.getByText(seed.title)).toBeInTheDocument();
  fireEvent.click(screen.getByText("Reveal connections"));
  expect(screen.getAllByText("1980s → 2010s")).toHaveLength(3);
  for (const track of candidates) expect(screen.getByText(track.title)).toBeInTheDocument();
  fireEvent.click(screen.getByText("Save discoveries"));
  expect(save).toHaveBeenCalledWith({ tracks: candidates, label: "Same sound, different world" });
  fireEvent.click(screen.getByText("Another round"));
  await screen.findByText("Candidate 1");
  expect(sonicWorlds).toHaveBeenLastCalledWith({ seedKey: seed.trackKey, boundary: "decade", direction: "cross", excludeKeys: candidates.map(t => t.trackKey) });
});

it("invalidates a pending round on control changes and ignores it after close", async () => {
  let finish!: (result: WorldResponse) => void;
  vi.mocked(sonicWorlds).mockReturnValue(new Promise(resolve => { finish = resolve; }));
  const panel = render(<SonicWorldsPanel {...controls} track={seed} onPlay={vi.fn()} onClose={vi.fn()} />);
  fireEvent.click(screen.getByText("Find three sounds"));
  fireEvent.click(screen.getByLabelText("Stay close"));
  await act(async () => finish(response));
  expect(screen.queryByText("Candidate 1")).not.toBeInTheDocument();
  expect(screen.getByText("Find three sounds")).toBeEnabled();
  fireEvent.change(screen.getByLabelText("Compare by"), { target: { value: "genre" } });
  fireEvent.click(screen.getByText("Find three sounds"));
  expect(sonicWorlds).toHaveBeenLastCalledWith({ seedKey: seed.trackKey, boundary: "genre", direction: "stay", excludeKeys: [] });
  panel.unmount();
  await act(async () => finish(response));
});

it("explains insufficient analysis, empty and partial rounds, and retryable errors", async () => {
  vi.mocked(sonicWorlds).mockRejectedValueOnce(new Error("The sound index is not ready"))
    .mockResolvedValueOnce({ ...response, seedReady: false, matches: [] })
    .mockResolvedValueOnce({ ...response, matches: [] })
    .mockResolvedValueOnce({ ...response, matches: response.matches.slice(0, 1) });
  render(<SonicWorldsPanel {...controls} track={seed} onPlay={vi.fn()} onClose={vi.fn()} />);
  fireEvent.click(screen.getByText("Find three sounds"));
  expect(await screen.findByRole("alert")).toHaveTextContent("index is not ready");
  fireEvent.click(screen.getByText("Find three sounds"));
  expect(await screen.findByRole("status")).toHaveTextContent("needs current, available audio analysis");
  fireEvent.click(screen.getByText("Find three sounds"));
  expect(await screen.findByRole("status")).toHaveTextContent("No close matches meet");
  fireEvent.click(screen.getByText("Find three sounds"));
  expect(await screen.findByRole("status")).toHaveTextContent("Only 1 close match meets");
  expect(screen.queryByText("Candidate 2")).not.toBeInTheDocument();
});
