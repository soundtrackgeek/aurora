import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as data from "../../artistPage";
import * as library from "../../library";
import { ArtistPage } from "./ArtistPage";

const discovery: data.ArtistDiscovery = { biography: "An artist biography.", listeners: 100, playCount: 9000, tags: ["indie"], topTracks: [{ name: "Missing song", playCount: 123456, listeners: 345, url: "https://www.last.fm/music/M83/_/Missing" }], similarArtists: [{ name: "Other Artist", matchScore: .8 }], warnings: [] };
const props = { artist: "M83", onBack: vi.fn(), onOpenArtist: vi.fn(), onOpenAlbum: vi.fn(), onPlay: vi.fn(async () => true), onSettings: vi.fn() };
beforeEach(() => {
  vi.spyOn(data, "loadArtistDiscovery").mockResolvedValue(discovery);
  vi.spyOn(data, "loadArtistArtwork").mockResolvedValue({ backgroundUrl: null, portraitUrl: null, warning: null });
  vi.spyOn(data, "loadArtistListening").mockResolvedValue({ plays: 7, listenedSeconds: 3600, months: {}, topTracks: [] });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.clearAllMocks(); });

it("renders independent MusicBrainz, personal history, and global Last.fm data without Follow", async () => {
  render(<ArtistPage {...props} />);
  expect(await screen.findByText("Antibes")).toBeInTheDocument();
  expect(screen.getByText("Founded")).toBeInTheDocument();
  expect(await screen.findByText("123,456")).toBeInTheDocument();
  expect(within(screen.getByRole("region", { name: "Your listening history" })).getByText("7")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /Follow/ })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Other Artist" }));
  expect(props.onOpenArtist).toHaveBeenCalledWith("Other Artist");
});

it("reveals library and MusicBrainz while the online provider is still pending", async () => {
  vi.mocked(data.loadArtistDiscovery).mockReturnValue(new Promise(() => {}));
  render(<ArtistPage {...props} />);
  expect(await screen.findByText("Antibes")).toBeInTheDocument();
  expect(screen.getByText("Loading popular tracks…")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Tracks" }));
  expect(await screen.findByRole("region", { name: "Tracks in your library" })).toBeInTheDocument();
});

it("keeps missing popular tracks visible and explains unavailable local playback", async () => {
  vi.spyOn(data, "findArtistTrack").mockResolvedValue(null);
  render(<ArtistPage {...props} />);
  fireEvent.click(await screen.findByRole("button", { name: /Missing song/ }));
  expect(await screen.findByRole("status")).toHaveTextContent("not available in your local library");
  expect(props.onPlay).not.toHaveBeenCalled();
});

it("does not commit the previous artist's delayed provider results after a keyed switch", async () => {
  let finish!: (value: data.ArtistDiscovery) => void;
  vi.mocked(data.loadArtistDiscovery).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  const view = render(<ArtistPage key="M83" {...props} />);
  await screen.findByText("Antibes");
  vi.mocked(data.loadArtistDiscovery).mockResolvedValue({ ...discovery, biography: "Second artist biography." });
  view.rerender(<ArtistPage key="Coldplay" {...props} artist="Coldplay" />);
  await screen.findByText("Second artist biography.");
  finish({ ...discovery, biography: "Stale biography" });
  await waitFor(() => expect(screen.queryByText("Stale biography")).not.toBeInTheDocument());
});

it("provides a usable page and retries after independent provider failures", async () => {
  vi.mocked(data.loadArtistArtwork).mockRejectedValue(new Error("Artwork offline"));
  vi.spyOn(library, "loadArtistDetail").mockRejectedValue(new Error("No local albums"));
  render(<ArtistPage {...props} />);
  expect(await screen.findByText("No albums by this artist are available in your library.")).toBeInTheDocument();
  expect(await screen.findByText("fanart.tv: Artwork offline")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Retry unavailable data" }));
  await waitFor(() => expect(data.loadArtistArtwork).toHaveBeenLastCalledWith("M83", true));
});
