import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import { browserPreview, type AlbumSummary } from "../../library";
import * as home from "../../universe";
import { Universe } from "./Universe";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

const track = browserPreview.tracks[0];
const album: AlbumSummary = {
  id: "near-final", title: "The Finish Line", artist: "Example", releaseYear: 2004, originalYear: 1999,
  totalTracks: 8, ratedTracks: 7, lovedTracks: 1, durationSeconds: 420, genre: null, rating: null, albumScore: 134,
};

function fixture() {
  vi.spyOn(home, "loadUniverseArtists").mockResolvedValue([{ id: "m83", name: "M83", plays: 5 }]);
  vi.spyOn(home, "loadNearFinalAlbums").mockResolvedValue([album]);
  vi.spyOn(home, "loadRecentAlbums").mockResolvedValue([{ ...album, title: "New Import", id: "imported" }]);
  vi.spyOn(home, "loadRediscoverTracks").mockResolvedValue([{ track, lastListenedAtMs: Date.UTC(2024, 6, 1) }]);
  return {
    period: "recent", catalogRevision: 1, historyRevision: 1, playbackBusy: false,
    playback: { currentTrack: track, queue: [track], status: "paused", positionSeconds: 91 },
    onPeriodChange: vi.fn(), onResume: vi.fn(), onSelectArtist: vi.fn(), onOpenAlbum: vi.fn(), onPlayTrack: vi.fn(),
  } satisfies ComponentProps<typeof Universe>;
}

it("opens the selected artist and albums, resumes the saved queue, and plays a rediscovery", async () => {
  const props = fixture();
  render(<Universe {...props} />);
  fireEvent.click(await screen.findByRole("button", { name: "Explore M83, 5 plays" }));
  expect(props.onSelectArtist).toHaveBeenCalledWith(expect.objectContaining({ id: "m83" }));

  const resume = within(screen.getByRole("region", { name: "Continue Listening" }));
  expect(resume.getByText("At 1:31 · 1 in queue")).toBeVisible();
  fireEvent.click(resume.getByRole("button", { name: "Resume" }));
  expect(props.onResume).toHaveBeenCalledOnce();

  fireEvent.click(await screen.findByRole("button", { name: "Open The Finish Line, 1 rating left" }));
  expect(props.onOpenAlbum).toHaveBeenCalledWith("near-final", "The Finish Line");
  fireEvent.click(await screen.findByRole("button", { name: "Open New Import" }));
  expect(props.onOpenAlbum).toHaveBeenLastCalledWith("imported", "New Import");

  fireEvent.click(await screen.findByRole("button", { name: `Play ${track.title}` }));
  expect(props.onPlayTrack).toHaveBeenCalledWith(track, [track]);
});

it("keeps the requested period when the previous period arrives late", async () => {
  const props = fixture();
  let resolveRecent!: (artists: home.UniverseArtist[]) => void;
  vi.mocked(home.loadUniverseArtists).mockImplementation((period) => period === "recent"
    ? new Promise((resolve) => { resolveRecent = resolve; })
    : Promise.resolve([{ id: "daft-punk", name: "Daft Punk", plays: 12 }]));
  const { rerender } = render(<Universe {...props} />);
  fireEvent.click(screen.getByRole("button", { name: "All time" }));
  expect(props.onPeriodChange).toHaveBeenCalledWith("all");
  rerender(<Universe {...props} period="all" />);
  expect(await screen.findByRole("button", { name: "Explore Daft Punk, 12 plays" })).toBeVisible();
  await act(async () => { resolveRecent([{ id: "m83", name: "M83", plays: 5 }]); });
  expect(screen.queryByRole("button", { name: "Explore M83, 5 plays" })).toBeNull();
});
