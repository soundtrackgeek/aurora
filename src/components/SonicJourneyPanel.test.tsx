import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SonicJourneyPanel } from "./SonicJourneyPanel";
import { useSonicJourney } from "../useSonicJourney";
import * as backend from "../sonicJourney";
vi.mock("../sonicJourney", () => ({ searchJourneyTracks: vi.fn(), buildSonicJourney: vi.fn(), saveSonicJourney: vi.fn() }));
function track(id: number): backend.JourneyTrack { return { trackId: id, trackKey: `key${id}`, title: `Song ${id}`, artist: "Singer", album: "Album", albumId: "album", albumArtist: "Various Artists", filePath: "Test", filename: `${id}.mp3`, genre: "Pop", rating: 80, seconds: 180, loved: false }; }
const stops = Array.from({ length: 5 }, (_, i) => track(i + 1));
afterEach(cleanup);
beforeEach(() => { vi.resetAllMocks(); vi.mocked(backend.searchJourneyTracks).mockResolvedValue(stops); vi.mocked(backend.saveSonicJourney).mockResolvedValue("Saved journey in Playlists."); });
function Harness({ onPlay, onSaved }: { onPlay?: (tracks: backend.JourneyTrack[]) => Promise<void>; onSaved?: () => void }) { const journey = useSonicJourney(onSaved); return <SonicJourneyPanel journey={journey} onPlay={onPlay} />; }
async function choose() { fireEvent.change(screen.getByLabelText("Find analyzed tracks"), { target: { value: "Song" } }); fireEvent.click(screen.getByText("Search stops")); await screen.findByRole("button", { name: "Add Song 1" }); for (const s of stops) fireEvent.click(screen.getByRole("button", { name: `Add ${s.title}` })); }
it("builds a five-stop journey, preserves reviewed order for save/play, and invalidates it on edits", async () => {
  let reviewed: backend.JourneyTrack[] = [];
  vi.mocked(backend.buildSonicJourney).mockImplementation(async request => {
    reviewed = request.stopKeys.flatMap((key, i) => [ ...(i ? [track(100 + i * 2), track(101 + i * 2)] : []), stops.find(t => t.trackKey === key)! ]);
    return { complete: true, analyzed: 120, stopsReady: request.stopKeys.map(() => true), tracks: reviewed };
  });
  const play = vi.fn().mockResolvedValue(undefined), saved = vi.fn(); render(<Harness onPlay={play} onSaved={saved} />);
  await choose(); fireEvent.click(screen.getByLabelText("Move Song 2 earlier"));
  fireEvent.change(screen.getByLabelText("Connecting tracks between stops"), { target: { value: "2" } });
  fireEvent.change(screen.getByLabelText("Connector minimum rating"), { target: { value: "80" } }); fireEvent.click(screen.getByLabelText("Connectors in first stop’s genre"));
  expect(screen.getByText("5 stops · 13 tracks including connectors")).toBeInTheDocument();
  fireEvent.click(screen.getByText("Build sonic journey")); await screen.findByText(/13 tracks through 5 stops/);
  expect(backend.buildSonicJourney).toHaveBeenCalledWith({ stopKeys: ["key2", "key1", "key3", "key4", "key5"], connectingTracks: 2, minimumRating: 80, sameGenre: true });
  fireEvent.click(screen.getByText("Play journey")); await waitFor(() => expect(play).toHaveBeenCalledWith(reviewed));
  fireEvent.change(screen.getByLabelText("Journey name"), { target: { value: "Five stops" } }); fireEvent.click(screen.getByText("Save journey playlist"));
  await screen.findByText("Saved journey in Playlists."); expect(backend.saveSonicJourney).toHaveBeenCalledWith(expect.objectContaining({ stopKeys: ["key2", "key1", "key3", "key4", "key5"] }), reviewed, "Five stops"); expect(saved).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByLabelText("Remove Song 3")); expect(screen.queryByText("Save journey playlist")).not.toBeInTheDocument();
});
it("reports unanalyzed stops and insufficient bridges without offering an incomplete playlist", async () => {
  vi.mocked(backend.buildSonicJourney).mockResolvedValueOnce({ complete: false, analyzed: 8, stopsReady: [true, false, true, true, true], tracks: [] }).mockResolvedValue({ complete: false, analyzed: 8, stopsReady: [true, true, true, true, true], tracks: [] });
  render(<Harness />); await choose(); fireEvent.click(screen.getByText("Build sonic journey")); await screen.findByText(/Stops 2 need current audio analysis/); expect(screen.queryByText("Save journey playlist")).not.toBeInTheDocument();
  fireEvent.click(screen.getByText("Build sonic journey")); await screen.findByText(/Not enough eligible connecting tracks/);
});
it("discards a late build after the journey settings change", async () => {
  let complete!: (response: backend.JourneyResponse) => void;
  vi.mocked(backend.buildSonicJourney).mockReturnValue(new Promise(resolve => { complete = resolve; }));
  const { result } = renderHook(() => useSonicJourney());
  act(() => result.current.add(stops[0])); act(() => result.current.add(stops[1]));
  act(() => { void result.current.build(); }); act(() => result.current.changeConnecting(1));
  await act(async () => complete({ complete: true, analyzed: 12, stopsReady: [true, true], tracks: stops }));
  expect(result.current.result).toBeNull(); expect(result.current.busy).toBe(false); expect(result.current.connecting).toBe(1);
});
