import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Track } from "../../library";
import type { RatingsOverview } from "../../ratings";
import { RatingsStudio } from "./RatingsStudio";

const album = { id: "album", title: "Almost There", artist: "Artist", originalYear: 2000, releaseYear: 2025, genre: "Rock", totalTracks: 10, ratedTracks: 8, lovedTracks: 1, durationSeconds: 2400, remainingTracks: 2, effectiveRating: null, provisionalRating: 4.25, albumScore: null };
const soundtrackTrack: Track = { id: "track", trackKey: "file:track", albumId: album.id, title: "Manhattan", artist: "Various Artists", displayArtist: "Andrea & Hot Mink", album: album.title, originalYear: 2000, releaseYear: 2025, rating: 4, loved: false, loveState: "neutral", tagSyncState: null, canUndoTagEdit: false, durationSeconds: 227, genre: "Soundtrack", playCount: 1 };
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
    refreshing: false,
    queueMessage: null,
    busyTrackKeys: new Set<string>(),
    onCompletionChange: vi.fn(), onSelectAlbum: vi.fn(), onGoToAlbum: vi.fn(), onSelectTrack: vi.fn(), onPlayTrack: vi.fn(),
    onRatingChange: vi.fn(), onLoveChange: vi.fn(), onPlayCollection: vi.fn(), onExploreCollection: vi.fn(),
    onPlayUnrated: vi.fn(), onRefresh: vi.fn(), onRetry: vi.fn(), onRetryPage: vi.fn(),
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
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    expect(callbacks.onRefresh).toHaveBeenCalledOnce();
    expect(screen.getByText("4.25 ★ provisional")).toBeInTheDocument();
    expect(screen.getByText("Available when the effective album rating is valid")).toBeInTheDocument();
    expect(screen.getByText(/2000 · Rock · 10 tracks/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Go to Album" }));
    expect(callbacks.onGoToAlbum).toHaveBeenCalledWith(album);
  });

  it("shows each track's display artist in album completion detail", () => {
    render(<RatingsStudio {...props()} albumTracks={[soundtrackTrack]} />);

    expect(screen.getByText("Manhattan")).toBeVisible();
    expect(screen.getByText("[Andrea & Hot Mink]")).toBeVisible();
    expect(screen.queryByText("[Various Artists]")).not.toBeInTheDocument();
  });

  it("shows refresh feedback until the reload completes", () => {
    const callbacks = props();
    const { container, rerender } = render(<RatingsStudio {...callbacks} />);
    const refresh = within(container).getByRole("button", { name: "Refresh" });

    fireEvent.click(refresh);
    expect(callbacks.onRefresh).toHaveBeenCalledOnce();

    rerender(<RatingsStudio {...callbacks} refreshing />);
    const refreshing = within(container).getByRole("button", { name: "Refreshing…" });
    expect(refreshing).toBeDisabled();
    expect(refreshing).toHaveAttribute("aria-busy", "true");
    expect(refreshing.querySelector("svg")).toHaveClass("is-spinning");

    rerender(<RatingsStudio {...callbacks} refreshing={false} />);
    expect(within(container).getByRole("button", { name: "Refresh" })).toBeEnabled();
  });
});
