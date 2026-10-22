import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AlbumSidesPanel } from "./AlbumSidesPanel";
import { exploreAlbumSound, type AlbumSidesResponse } from "../sonic";
vi.mock("../sonic", () => ({ exploreAlbumSound: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const track = { trackKey: "one", albumId: "seed", title: "Opening", artist: "Composer" };
const second = { ...track, trackKey: "two", title: "Quiet room" };
const contrast = { ...track, trackKey: "three", title: "Final battle" };
const album = { albumId: "other", title: "Another score", albumArtist: "Other composer", genre: "Score", analyzedTracks: 4, totalTracks: 4, distance: 0.1 };
const match = { album, matchingTrack: { ...track, albumId: "other", trackKey: "match", title: "A hidden cue" }, distance: 0.1 };
const result: AlbumSidesResponse = { version: "v1", ready: true, totalTracks: 3, analyzedTracks: 3,
  sides: [{ representative: track, tracks: [track, second] }, { representative: contrast, tracks: [contrast] }], contrastingTrack: contrast, selectedKey: null, matches: [], searchLimited: false };
const props = { albumId: "seed", onOpenAlbum: vi.fn(), onPlay: vi.fn().mockResolvedValue(undefined) };
it("explores only on request, plays groups, and discovers albums through a matching cue", async () => {
  vi.mocked(exploreAlbumSound).mockResolvedValueOnce(result).mockResolvedValueOnce({ ...result, selectedKey: "one", matches: [match], searchLimited: true });
  render(<AlbumSidesPanel {...props} />);
  expect(exploreAlbumSound).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Explore its sound" }));
  expect(await screen.findByText("Side 1 · 2 tracks")).toBeInTheDocument();
  fireEvent.click(screen.getByText("Tracks close to this one (2)"));
  expect(screen.getByText("Quiet room")).toBeVisible();
  fireEvent.click(screen.getByText("Play this side"));
  await waitFor(() => expect(props.onPlay).toHaveBeenCalledWith([track, second]));
  fireEvent.click(screen.getByLabelText("Find albums containing the sound of Opening"));
  expect(await screen.findByText("Another score")).toBeInTheDocument();
  expect(exploreAlbumSound).toHaveBeenLastCalledWith("seed", "one");
  fireEvent.click(screen.getByText("Another score")); expect(props.onOpenAlbum).toHaveBeenCalledWith(album);
  fireEvent.click(screen.getByLabelText("Play matching track A hidden cue"));
  await waitFor(() => expect(props.onPlay).toHaveBeenCalledWith([match.matchingTrack]));
  expect(screen.getByText(/not an exhaustive ranking/)).toBeInTheDocument();
});
it("shows incomplete coverage without offering invented groups", async () => {
  vi.mocked(exploreAlbumSound).mockResolvedValue({ ...result, ready: false, analyzedTracks: 2, sides: [], contrastingTrack: null });
  render(<AlbumSidesPanel {...props} />); fireEvent.click(screen.getByRole("button", { name: "Explore its sound" }));
  expect(await screen.findByRole("status")).toHaveTextContent("2/3");
  expect(screen.getByRole("status")).toHaveTextContent("Complete analysis is needed");
  expect(screen.queryByText("Play this side")).not.toBeInTheDocument();
});
it("discards a late result after album navigation", async () => {
  let finish!: (r: AlbumSidesResponse) => void;
  vi.mocked(exploreAlbumSound).mockReturnValue(new Promise(resolve => { finish = resolve; }));
  const panel = render(<AlbumSidesPanel {...props} />);
  fireEvent.click(screen.getByRole("button", { name: "Explore its sound" }));
  panel.rerender(<AlbumSidesPanel {...props} albumId="next" />);
  await act(async () => finish(result));
  expect(screen.queryByText("Opening")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Explore its sound" })).toBeEnabled();
});
it("reports lookup and playback failures and allows retry", async () => {
  vi.mocked(exploreAlbumSound).mockRejectedValueOnce(new Error("Analysis unavailable")).mockResolvedValueOnce({ ...result, sides: [result.sides[0]], contrastingTrack: null });
  render(<AlbumSidesPanel {...props} onPlay={vi.fn().mockRejectedValue(new Error("File unavailable"))} />);
  fireEvent.click(screen.getByRole("button", { name: "Explore its sound" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Analysis unavailable");
  fireEvent.click(screen.getByRole("button", { name: "Explore its sound" }));
  expect(await screen.findByRole("status")).toHaveTextContent("One representative for this album");
  fireEvent.click(screen.getByLabelText("Play Opening"));
  expect(await screen.findByRole("alert")).toHaveTextContent("File unavailable");
});
it("keeps groups available when discovery needs an index retry", async () => {
  vi.mocked(exploreAlbumSound).mockResolvedValueOnce(result)
    .mockRejectedValueOnce(new Error("The sound search index is not ready yet."))
    .mockResolvedValueOnce({ ...result, selectedKey: "one", matches: [match] });
  render(<AlbumSidesPanel {...props} />);
  fireEvent.click(screen.getByRole("button", { name: "Explore its sound" }));
  await screen.findByText("Side 1 · 2 tracks");
  fireEvent.click(screen.getByLabelText("Find albums containing the sound of Opening"));
  expect(await screen.findByRole("alert")).toHaveTextContent("index is not ready");
  expect(screen.getByText("Play this side")).toBeEnabled();
  fireEvent.click(screen.getByLabelText("Find albums containing the sound of Opening"));
  expect(await screen.findByText("Another score")).toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});
