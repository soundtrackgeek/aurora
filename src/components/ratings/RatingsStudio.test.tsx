import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RatingsOverview } from "../../ratings";
import { RatingsStudio } from "./RatingsStudio";

const album = { id: "album", title: "Almost There", artist: "Artist", originalYear: 2000, releaseYear: 2025, genre: "Rock", totalTracks: 10, ratedTracks: 8, lovedTracks: 1, durationSeconds: 2400, remainingTracks: 2, effectiveRating: null, provisionalRating: 4.25, albumScore: null };
const overview: RatingsOverview = {
  trackBands: [null, .5, 1, 1.5, 2, 2.5, 3, 3.5, 4, 4.5, 5].map((rating) => ({ rating, count: rating === null ? 947_794 : rating === 5 ? 59_293 : 10 })),
  albumBands: [null, .5, 1, 1.5, 2, 2.5, 3, 3.5, 4, 4.5, 5].map((rating) => ({ rating, count: rating === 5 ? 1_120 : 5 })),
  completion: { almostComplete: 678, partiallyRated: 5_723, unrated: 59_578 },
  ratedAlbums: 12_434,
  fiveStarAlbums: [{ ...album, ratedTracks: 10, remainingTracks: 0, effectiveRating: 5, albumScore: 112 }],
  initialPage: { kind: "almostComplete", total: 678, albums: [album] },
};

function props() {
  return {
    overview,
    page: overview.initialPage,
    selectedAlbum: album,
    albumTracks: [],
    loadState: "ready" as const,
    pageState: "ready" as const,
    errorMessage: null,
    pageError: null,
    queueBusy: false,
    queueMessage: null,
    busyTrackKeys: new Set<string>(),
    onCompletionChange: vi.fn(), onSelectAlbum: vi.fn(), onSelectTrack: vi.fn(), onPlayTrack: vi.fn(),
    onRatingChange: vi.fn(), onLoveChange: vi.fn(), onPlayCollection: vi.fn(), onExploreCollection: vi.fn(),
    onPlayUnrated: vi.fn(), onRetry: vi.fn(), onRetryPage: vi.fn(),
  };
}

describe("RatingsStudio", () => {
  it("keeps track ratings, album ratings, and completion distinct", () => {
    const callbacks = props();
    render(<RatingsStudio {...callbacks} />);
    expect(screen.getByText("947,794")).toBeInTheDocument();
    expect(document.querySelectorAll(".constellation-pyramid")).toHaveLength(6);
    expect(document.querySelectorAll(".constellation-band:last-child .constellation-pyramid__row")).toHaveLength(7);
    expect(document.querySelectorAll(".constellation-band:last-child .constellation-cover")).toHaveLength(28);
    fireEvent.click(screen.getByRole("tab", { name: "Album ratings" }));
    expect(screen.getByLabelText("5 stars, 1,120")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: /Partially rated/ }));
    expect(callbacks.onCompletionChange).toHaveBeenCalledWith("partiallyRated");
    expect(screen.getByText("4.25 ★ provisional")).toBeInTheDocument();
    expect(screen.getByText("Available when the effective album rating is valid")).toBeInTheDocument();
    expect(screen.getByText(/2000 · Rock · 10 tracks/)).toBeInTheDocument();
  });
});
