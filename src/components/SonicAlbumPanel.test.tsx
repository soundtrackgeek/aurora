import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { SonicAlbumPanel } from "./SonicAlbumPanel";
import { sonicAlbumMatches, type SonicAlbumMatches } from "../sonic";
vi.mock("../sonic", () => ({ sonicAlbumMatches: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const response: SonicAlbumMatches = { seed: { albumId: "seed", title: "Seed", albumArtist: "Singer", genre: "Pop", totalTracks: 6, analyzedTracks: 3, distance: null }, seedReady: true, analyzedAlbums: 2,
  albums: [{ albumId: "other", title: "Other album", albumArtist: "Various Artists", genre: "Pop", totalTracks: 8, analyzedTracks: 4, distance: 0.2 }] };
const props = { albumId: "seed", onOpenAlbum: vi.fn(), onPlayAlbum: vi.fn().mockResolvedValue(undefined), onRadio: vi.fn(), radioBusy: false, radioMessage: null, radioActive: false, onStopRadio: vi.fn() };
it("shows coverage, opens/plays matches, and starts radio from the album", async () => {
  vi.mocked(sonicAlbumMatches).mockResolvedValue(response);
  render(<SonicAlbumPanel {...props} />);
  fireEvent.click(screen.getByText("Find similar albums"));
  expect(await screen.findByText("4/8 tracks analyzed · Partial analysis")).toBeInTheDocument();
  fireEvent.click(screen.getByText("Other album")); expect(props.onOpenAlbum).toHaveBeenCalledWith(response.albums[0]);
  fireEvent.click(screen.getByLabelText("Play Other album")); await waitFor(() => expect(props.onPlayAlbum).toHaveBeenCalledWith(response.albums[0]));
  await waitFor(() => expect(screen.getByText("Start album sonic radio")).toBeEnabled());
  fireEvent.change(screen.getByLabelText("Radio minimum rating"), { target: { value: "4" } });
  fireEvent.click(screen.getByLabelText("Radio same genre")); fireEvent.click(screen.getByText("Start album sonic radio"));
  expect(props.onRadio).toHaveBeenCalledWith(response.seed, 4, true, 50);
  fireEvent.change(screen.getByLabelText("Minimum analysis coverage"), { target: { value: "100" } });
  expect(screen.queryByText("Start album sonic radio")).not.toBeInTheDocument();
});
it("ignores late matches from a previous album selection", async () => {
  let finish!: (r: SonicAlbumMatches) => void;
  vi.mocked(sonicAlbumMatches).mockReturnValue(new Promise(resolve => { finish = resolve; }));
  const panel=render(<SonicAlbumPanel {...props} />);
  fireEvent.click(screen.getByText("Find similar albums"));
  panel.rerender(<SonicAlbumPanel {...props} albumId="next" />);
  await act(async () => finish(response));
  expect(screen.queryByText("Other album")).not.toBeInTheDocument();
});
it("offers guidance for insufficient analysis and reports errors", async () => {
  vi.mocked(sonicAlbumMatches).mockResolvedValueOnce({ ...response, seedReady: false, albums: [] }).mockRejectedValueOnce(new Error("Query failed"));
  render(<SonicAlbumPanel {...props} />); fireEvent.click(screen.getByText("Find similar albums"));
  expect(await screen.findByRole("status")).toHaveTextContent("Analyze this album in Music Library");
  expect(screen.queryByText("Start album sonic radio")).not.toBeInTheDocument();
  fireEvent.click(screen.getByText("Find similar albums"));
  expect(await screen.findByRole("alert")).toHaveTextContent("Query failed");
});
