import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as data from "../../artistPage";
import * as charts from "../../charts";
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

it("shows albums, release years, and playable tracks when the artist link uses different casing", async () => {
  render(<ArtistPage {...props} artist="COLDPLAY" />);
  expect(await screen.findByRole("button", { name: "Open Viva la Vida" })).toBeInTheDocument();
  expect(screen.queryByText("No albums by this artist are available in your library.")).not.toBeInTheDocument();
  expect(within(screen.getByRole("region", { name: "Release timeline" })).getByText("2008")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Albums" }));
  fireEvent.click(screen.getByRole("button", { name: "Open Viva la Vida" }));
  expect(props.onOpenAlbum).toHaveBeenCalledWith(expect.objectContaining({ title: "Viva la Vida", artist: "Coldplay" }));
  fireEvent.click(screen.getByRole("button", { name: "Tracks" }));
  const track = await screen.findByRole("button", { name: /Strawberry Swing/ });
  expect(screen.getByText("1 of 1")).toBeInTheDocument();
  fireEvent.click(track);
  await waitFor(() => expect(props.onPlay).toHaveBeenCalledWith([expect.objectContaining({ title: "Strawberry Swing", artist: "Coldplay" })]));
});

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

it("shows loved tracks and a complete chart overview for albums and tracks", async () => {
  const history: charts.ArtistChartHistory = {
    singles: [
      { title: "Late Hit", bestPeak: 1, firstYear: 1990, trackId: null, albumId: null, peaks: [{ label: "VG Lista", country: "NO", peak: 1, weeks: 9, annual: false }] },
      { title: "Early Song", bestPeak: 24, firstYear: 1985, trackId: null, albumId: null, peaks: [{ label: "Official UK", country: "UK", peak: 24, weeks: 3, annual: false }, { label: "Billboard annual", country: "US", peak: 80, weeks: 1, annual: true }] },
    ],
    albums: [{ title: "Charted Album", bestPeak: 3, firstYear: 1986, trackId: null, albumId: null, peaks: [{ label: "VG Lista", country: "NO", peak: 3, weeks: 12, annual: false }] }],
  };
  vi.spyOn(charts, "loadArtistChartHistory").mockResolvedValue(history);
  const lovedTrack = { ...(await library.exploreTracks({ artist: "Coldplay", pageSize: 1 }, { localOnly: true })).items[0], loved: true, loveState: "loved" as const };
  const explore = library.exploreTracks;
  vi.spyOn(library, "exploreTracks").mockImplementation((request, options) => request.search === "love:1" ? Promise.resolve({ items: [lovedTrack], nextCursor: null, totalCount: 1 }) : explore(request, options));
  render(<ArtistPage {...props} artist="Coldplay" />);

  const lovedPanel = await screen.findByRole("region", { name: "Loved tracks" });
  expect(await within(lovedPanel).findByText(lovedTrack.title)).toBeInTheDocument();
  expect(within(lovedPanel).getByText("1 loved track in your library")).toBeInTheDocument();
  fireEvent.click(within(lovedPanel).getByRole("button", { name: "Play all" }));
  await waitFor(() => expect(props.onPlay).toHaveBeenCalledWith([expect.objectContaining({ title: lovedTrack.title })]));

  const tracksPanel = await screen.findByRole("region", { name: "Charted tracks" });
  expect(await within(tracksPanel).findByText("2 charted tracks · 1 top 10 · 1 number one")).toBeInTheDocument();
  const titles = () => within(tracksPanel).getAllByRole("listitem").map((item) => item.querySelector("strong")?.textContent);
  expect(titles()).toEqual(["Early Song", "Late Hit"]);
  fireEvent.click(within(tracksPanel).getByRole("button", { name: "Peak" }));
  expect(titles()).toEqual(["Late Hit", "Early Song"]);
  expect(within(tracksPanel).getByTitle("Billboard annual: peak #80 · 1 year-end chart")).toBeInTheDocument();

  const albumsPanel = screen.getByRole("region", { name: "Charted albums" });
  expect(within(albumsPanel).getByText(/Not in your library/)).toBeInTheDocument();
  expect(within(albumsPanel).queryByRole("button", { name: /Charted Album/ })).not.toBeInTheDocument();
});
