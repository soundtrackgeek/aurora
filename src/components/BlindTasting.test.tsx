import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { browserPreview } from "../library";
import { sonicWorlds } from "../sonicWorlds";
import { blindTastingPlayback, getPlaybackSnapshot } from "../playback";
import { BlindTasting } from "./BlindTasting";
import { PlaylistAuthoringContext } from "./playlists/playlistAuthoringContext";
vi.mock("../sonicWorlds", () => ({ sonicWorlds: vi.fn() }));
vi.mock("../playback", async load => ({ ...await load<typeof import("../playback")>(), blindTastingPlayback: vi.fn() }));
const seed = browserPreview.tracks[0], pair = browserPreview.tracks.slice(1, 3);
beforeEach(async () => {
  HTMLDialogElement.prototype.showModal = vi.fn(function (this: HTMLDialogElement) { this.open = true; });
  vi.mocked(blindTastingPlayback).mockResolvedValue(await getPlaybackSnapshot());
  vi.mocked(sonicWorlds).mockResolvedValue({ version: "v1", seedReady: true, seedValue: null, maxDistance: 0.25, searchLimited: true, matches: pair.map(track => ({ track, value: null, distance: 0.1 })) });
});
afterEach(() => { cleanup(); vi.resetAllMocks(); });

it("conceals all candidate identity until choosing, saves explicitly, rotates and stops on close", async () => {
  const save = vi.fn(), close = vi.fn();
  render(<PlaylistAuthoringContext value={save}><BlindTasting seed={seed} playback={await getPlaybackSnapshot()} onSnapshot={vi.fn()} onPause={vi.fn()} onSeek={vi.fn()} onAlbum={vi.fn()} onClose={close} /></PlaylistAuthoringContext>);
  fireEvent.click(screen.getByText("Prepare a round"));
  await screen.findByText("Play A");
  pair.forEach(t => expect(screen.queryByText(t.title)).not.toBeInTheDocument());
  expect(screen.getByText("Keep A playing · reveal")).toBeDisabled();
  fireEvent.click(screen.getByText("Play A")); await waitFor(() => expect(screen.getByText("Play B")).toBeEnabled());
  fireEvent.click(screen.getByText("Play B")); await waitFor(() => expect(screen.getByText("Keep A playing · reveal")).toBeEnabled());
  expect(blindTastingPlayback).toHaveBeenCalledWith(expect.objectContaining({ trackKey: expect.any(String) }), "Candidate A");
  fireEvent.click(screen.getByText("Keep A playing · reveal"));
  await waitFor(() => expect(screen.getAllByText("Save")[0]).toBeEnabled());
  pair.forEach(t => expect(screen.getByText(t.title)).toBeInTheDocument());
  expect(save).not.toHaveBeenCalled();
  fireEvent.click(screen.getAllByText("Save")[0]); expect(save).toHaveBeenCalledWith({ tracks: [expect.objectContaining({ trackKey: expect.any(String) })], label: expect.any(String) });
  fireEvent.click(screen.getByText("Another round")); await screen.findByText("Play A");
  expect(sonicWorlds).toHaveBeenLastCalledWith(expect.objectContaining({ excludeKeys: expect.arrayContaining(pair.map(t => t.trackKey)) }));
  fireEvent.click(screen.getByText("Close")); await waitFor(() => expect(close).toHaveBeenCalled());
  expect(blindTastingPlayback).toHaveBeenLastCalledWith();
});

it("rejects partial rounds and discards a late response after unmount", async () => {
  vi.mocked(sonicWorlds).mockResolvedValueOnce({ version: "v1", seedReady: true, seedValue: null, maxDistance: 0.25, searchLimited: true, matches: [] });
  const props = { seed, playback: await getPlaybackSnapshot(), onSnapshot: vi.fn(), onPause: vi.fn(), onSeek: vi.fn(), onAlbum: vi.fn(), onClose: vi.fn() };
  const view = render(<BlindTasting {...props} />);
  fireEvent.click(screen.getByText("Prepare a round")); expect(await screen.findByRole("alert")).toHaveTextContent("not two");
  let finish!: (value: Awaited<ReturnType<typeof sonicWorlds>>) => void;
  vi.mocked(sonicWorlds).mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
  fireEvent.click(screen.getByText("Prepare a round")); await waitFor(() => expect(sonicWorlds).toHaveBeenCalledTimes(2)); view.unmount();
  await act(async () => finish({ version: "v1", seedReady: false, seedValue: null, maxDistance: 0.25, searchLimited: true, matches: [] }));
});
