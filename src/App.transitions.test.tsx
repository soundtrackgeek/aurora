import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import App from "./App";
import * as library from "./library";
import * as tags from "./tags";

// Canvas playback rendering is unrelated to album request/commit ordering.
vi.mock("./components/WaveformTimeline", () => ({ WaveformTimeline: () => null }));

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  localStorage.clear();
});

async function openAlbums() {
  // The sidebar mounts before the snapshot that enables destination routes.
  await screen.findByRole("region", { name: "Library overview" });
  const primary = within(screen.getByRole("navigation", { name: "Primary" }));
  fireEvent.click(primary.getByRole("button", { name: "Albums" }));
  await act(async () => {
    // Include cold route imports in act, before starting the DOM query timeout.
    await vi.dynamicImportSettled();
  });
}

it("commits local album details before starting file reconciliation", async () => {
  const load = library.loadAlbumDetail;
  const committedDetails: boolean[] = [];
  vi.spyOn(library, "loadAlbumDetail").mockImplementation((albumId, options) => {
    if (!options?.localOnly) {
      committedDetails.push(screen.queryByRole("complementary", { name: "Viva la Vida album details" }) !== null);
    }
    return load(albumId, options);
  });
  render(<App />);
  await openAlbums();
  fireEvent.click(await screen.findByRole("button", { name: /^Viva la Vida cover/ }));
  await waitFor(() => expect(committedDetails).toEqual([true]));
  fireEvent.click(screen.getByRole("button", { name: "Close album details" }));
  await waitFor(() => expect(screen.queryByRole("complementary", { name: "Viva la Vida album details" })).not.toBeInTheDocument());
});

it("keeps the new album's details and selected track when a pending rating save completes", async () => {
  const older = await library.loadAlbumDetail("preview-viva");
  const track = older.tracks[0];
  const rating = track.rating === 3 ? 2 : 3;
  let finish!: (value: tags.TrackTagSnapshot) => void;
  const update = vi.spyOn(tags, "updateTrackTags").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  render(<App />);
  await openAlbums();
  fireEvent.click(await screen.findByRole("button", { name: /^Viva la Vida cover/ }));
  const details = await screen.findByRole("complementary", { name: "Viva la Vida album details" });
  fireEvent.click(within(details).getByRole("button", { name: `Rate ${track.title} ${rating.toFixed(1)} stars` }));
  await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
  expect(within(details).getByText("Pending tag import")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: /^Hurry Up, We're Dreaming cover/ }));
  const newerDetails = await screen.findByRole("complementary", { name: "Hurry Up, We're Dreaming album details" });
  fireEvent.click(within(newerDetails).getByRole("row", { name: /Midnight City/ }));
  expect(within(newerDetails).getByRole("row", { name: /Midnight City/ })).toHaveAttribute("aria-selected", "true");

  await act(async () => {
    finish({
      track: { ...track, rating, tagSyncState: null },
      tagState: { values: { ...tags.tagValuesForTrack(track), rating }, syncState: null, canUndo: false },
      catalogSync: { status: "synced", pendingFolderCount: 0 },
    });
  });
  const currentDetails = screen.getByRole("complementary", { name: "Hurry Up, We're Dreaming album details" });
  expect(within(currentDetails).getByRole("row", { name: /Midnight City/ })).toHaveAttribute("aria-selected", "true");
  expect(within(currentDetails).queryByRole("row", { name: /Strawberry Swing/ })).not.toBeInTheDocument();
  expect(screen.queryByRole("complementary", { name: "Viva la Vida album details" })).not.toBeInTheDocument();
});

it("reveals artist intelligence without waiting for the catalog summary", async () => {
  const detail = await library.loadArtistDetail("M83");
  let finishCatalog!: (value: library.ArtistDetail) => void;
  vi.spyOn(library, "loadArtistDetail").mockImplementation(() => new Promise((resolve) => { finishCatalog = resolve; }));
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Explore M83, 94 tracks" }));
  expect(await screen.findByRole("region", { name: "MusicBrainz identity status" })).toBeInTheDocument();
  expect(screen.queryByLabelText("Local catalog summary")).not.toBeInTheDocument();
  finishCatalog(detail);
  expect(await screen.findByLabelText("Local catalog summary")).toHaveTextContent("94");
});

it("opens an album artist page and returns to the same Albums results", async () => {
  render(<App />);
  await openAlbums();
  const cover = await screen.findByRole("button", { name: /^Viva la Vida cover/ });
  fireEvent.click(cover);
  const details = await screen.findByRole("complementary", { name: "Viva la Vida album details" });
  fireEvent.click(within(details).getByRole("button", { name: "Open artist page for Coldplay" }));
  expect(await screen.findByRole("article", { name: "Coldplay artist page" }, { timeout: 5000 })).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "Search your music universe" })).toHaveValue("");
  fireEvent.click(screen.getByRole("button", { name: "Back to Albums" }));
  expect(await screen.findByRole("complementary", { name: "Viva la Vida album details" })).toBeInTheDocument();
  fireEvent.click(within(screen.getByRole("complementary", { name: "Viva la Vida album details" })).getByRole("button", { name: "Open artist page for Coldplay" }));
  fireEvent.click(await screen.findByRole("button", { name: "Open Viva la Vida" }));
  expect(await screen.findByRole("complementary", { name: "Viva la Vida album details" })).toBeInTheDocument();
});
