import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import App from "./App";
import * as library from "./library";

// Canvas playback rendering is unrelated to album request/commit ordering.
vi.mock("./components/WaveformTimeline", () => ({ WaveformTimeline: () => null }));

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  localStorage.clear();
});

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
  fireEvent.click(await screen.findByRole("button", { name: "Albums" }));
  fireEvent.click(await screen.findByRole("button", { name: /^Viva la Vida cover/ }));
  await waitFor(() => expect(committedDetails).toEqual([true]));
  fireEvent.click(screen.getByRole("button", { name: "Close album details" }));
  await waitFor(() => expect(screen.queryByRole("complementary", { name: "Viva la Vida album details" })).not.toBeInTheDocument());
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
