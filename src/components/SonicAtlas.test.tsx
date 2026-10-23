import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SonicAtlas } from "./SonicAtlas";
import { demoAtlas, operateSonicAtlas, searchAtlasTracks, type SonicAtlas as AtlasData, type SonicTrack } from "../sonicAtlas";
vi.mock("../sonicAtlas", async importOriginal => ({ ...await importOriginal<typeof import("../sonicAtlas")>(), operateSonicAtlas: vi.fn(), searchAtlasTracks: vi.fn() }));
const tracks: SonicTrack[] = Array.from({length: 9}, (_,i) => ({ trackId:i+1, trackKey:`key-${i}`, title:`Track ${i}`, artist:`Artist ${i}`, album:`Album ${i}`, albumArtist:`Artist ${i}`, albumId:`album-${i}`, filePath:"Preview", filename:`${i}.mp3`, genre:null, rating:i%2 ? null : 100, seconds:180, loved:i===0, distance:i/10 }));
const seed = { trackKey: tracks[0].trackKey, title: tracks[0].title };
const fixture = () => demoAtlas(tracks, seed.trackKey);
beforeEach(() => {
  HTMLDialogElement.prototype.showModal = vi.fn(function(this: HTMLDialogElement) { this.open = true; });
  vi.mocked(operateSonicAtlas).mockImplementation(async request => ({ ...demoAtlas(tracks, request.seedKey), message:request.action === "save" ? "Saved region." : null }));
});
afterEach(() => { cleanup(); vi.resetAllMocks(); });
it("selects regions, validates playback, opens albums and saves the exact reviewed membership", async () => {
  const play=vi.fn().mockResolvedValue(undefined), open=vi.fn(), close=vi.fn();
  render(<SonicAtlas initialSeed={seed} onClose={close} onOpenAlbum={open} onPlay={play} />);
  const regions = await screen.findByRole("navigation", {name:"Sound regions"});
  fireEvent.click(within(regions).getByRole("button", {name:/Near Track 1/}));
  expect(within(regions).getByRole("button", {name:/Near Track 1/})).toHaveAttribute("aria-pressed", "true");
  fireEvent.click(screen.getByText("Play entry point"));
  await waitFor(() => expect(play).toHaveBeenCalledWith([tracks[1]]));
  expect(operateSonicAtlas).toHaveBeenLastCalledWith(expect.objectContaining({action:"audition",regionId:"key-1",trackKeys:["key-1"]}));
  fireEvent.click(screen.getByText("Save region playlist"));
  await screen.findByText("Saved region.");
  expect(operateSonicAtlas).toHaveBeenLastCalledWith(expect.objectContaining({action:"save",revision:`preview-${seed.trackKey}`,regionId:"key-1",trackKeys:["key-1","key-4","key-7"]}));
  fireEvent.click(screen.getByText("Open album")); expect(open).toHaveBeenCalledWith(tracks[1]); expect(close).toHaveBeenCalled();
});
it("reopens stable neighborhoods, returns back, and rebuilds only on explicit action", async () => {
  render(<SonicAtlas initialSeed={seed} onClose={vi.fn()} onOpenAlbum={vi.fn()} />);
  await screen.findByRole("navigation",{name:"Sound regions"});
  fireEvent.click(screen.getByRole("button", {name:"Center on Track 3"}));
  await waitFor(() => expect(screen.getByText("Back")).toBeEnabled());
  expect(operateSonicAtlas).toHaveBeenLastCalledWith(expect.objectContaining({action:"load",seedKey:"key-3"}));
  fireEvent.click(screen.getByText("Back"));
  await waitFor(() => expect(screen.getByText("Back")).toBeDisabled());
  fireEvent.click(screen.getByText("Reload counts"));
  await waitFor(() => expect(screen.getByText("Rebuild neighborhood")).toBeEnabled());
  expect(operateSonicAtlas).toHaveBeenLastCalledWith(expect.objectContaining({action:"load",seedKey:"key-0"}));
  fireEvent.click(screen.getByText("Rebuild neighborhood"));
  await waitFor(() => expect(operateSonicAtlas).toHaveBeenLastCalledWith(expect.objectContaining({action:"rebuild"})));
});
it("does not start playback after closing during a delayed validation", async () => {
  const play=vi.fn();
  const view=render(<SonicAtlas initialSeed={seed} onClose={vi.fn()} onOpenAlbum={vi.fn()} onPlay={play} />);
  await screen.findByText("Play entry point");
  let finish!: (a: AtlasData) => void;
  vi.mocked(operateSonicAtlas).mockReturnValueOnce(new Promise(resolve => { finish=resolve; }));
  fireEvent.click(screen.getByText("Play entry point")); view.unmount();
  await act(async () => finish(fixture())); expect(play).not.toHaveBeenCalled();
});
it("excludes unavailable points from actions and reports failed validation without playing", async () => {
  const a=fixture(); a.regions[0].points[0].available=false;
  vi.mocked(operateSonicAtlas).mockResolvedValueOnce(a);
  const play=vi.fn();
  render(<SonicAtlas initialSeed={seed} onClose={vi.fn()} onOpenAlbum={vi.fn()} onPlay={play} />);
  expect(await screen.findByRole("button",{name:"Play Track 0"})).toBeDisabled();
  vi.mocked(operateSonicAtlas).mockRejectedValueOnce(new Error("Recording changed"));
  fireEvent.click(screen.getByText("Play region"));
  expect(await screen.findByRole("alert")).toHaveTextContent("Recording changed"); expect(play).not.toHaveBeenCalled();
});
it("searches analyzed tracks and opens the chosen neighborhood", async () => {
  vi.mocked(searchAtlasTracks).mockResolvedValue([tracks[2]]);
  render(<SonicAtlas onClose={vi.fn()} onOpenAlbum={vi.fn()} />);
  fireEvent.change(screen.getByLabelText("Find an analyzed track"),{target:{value:"Artist"}});
  fireEvent.click(screen.getByText("Search tracks"));
  fireEvent.click(await screen.findByRole("button",{name:/Track 2 Artist 2/}));
  await screen.findByRole("navigation",{name:"Sound regions"});
  expect(operateSonicAtlas).toHaveBeenCalledWith(expect.objectContaining({action:"load",seedKey:"key-2"}));
});
it("retries a cold index without requiring a different starting track", async () => {
  vi.mocked(operateSonicAtlas).mockRejectedValueOnce(new Error("The sound index is not ready"));
  render(<SonicAtlas initialSeed={seed} onClose={vi.fn()} onOpenAlbum={vi.fn()} />);
  fireEvent.click(await screen.findByText("Retry neighborhood"));
  await screen.findByRole("navigation", {name:"Sound regions"});
  expect(operateSonicAtlas).toHaveBeenLastCalledWith(expect.objectContaining({action:"load",seedKey:seed.trackKey}));
});
it("keeps pause available inside the modal while playing", () => {
  const pause=vi.fn();
  const view=render(<SonicAtlas playing onPause={pause} onClose={vi.fn()} onOpenAlbum={vi.fn()} />);
  fireEvent.click(screen.getByText("Pause playback")); expect(pause).toHaveBeenCalledOnce();
  view.rerender(<SonicAtlas playing={false} onPause={pause} onClose={vi.fn()} onOpenAlbum={vi.fn()} />);
  expect(screen.getByText("Pause playback")).toBeDisabled();
});
