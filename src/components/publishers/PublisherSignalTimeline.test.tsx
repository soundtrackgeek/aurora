import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PublisherOverview } from "../../publishers";
import { PublisherSignalTimeline } from "./PublisherSignalTimeline";

afterEach(cleanup);

const overview: PublisherOverview = {
  publishers: [{
    name: "Parlophone",
    albumCount: 2,
    trackCount: 26,
    firstYear: 1966,
    lastYear: 1997,
    releaseActivity: [{ year: 1966, albumCount: 1, trackCount: 14 }, { year: 1997, albumCount: 1, trackCount: 12 }],
    originalActivity: [{ year: 1966, albumCount: 1, trackCount: 14 }, { year: 1997, albumCount: 1, trackCount: 12 }],
    logoUrl: null,
  }],
  initialDetail: {
    publisher: {
      name: "Parlophone",
      albumCount: 2,
      trackCount: 26,
      firstYear: 1966,
      lastYear: 1997,
      releaseActivity: [{ year: 1966, albumCount: 1, trackCount: 14 }],
      originalActivity: [{ year: 1966, albumCount: 1, trackCount: 14 }],
      logoUrl: null,
    },
    albums: [{
      id: "revolver",
      title: "Revolver",
      artist: "The Beatles",
      originalYear: 1966,
      releaseYear: 1966,
      publisher: "Parlophone",
      totalTracks: 14,
      ratedTracks: 14,
      lovedTracks: 8,
      durationSeconds: 2107,
      genre: "Rock",
      rating: 5,
    }],
  },
};

describe("PublisherSignalTimeline", () => {
  it("exposes timeline modes and opens publisher and album selections", () => {
    const onSelectPublisher = vi.fn();
    const onSelectAlbum = vi.fn();
    render(<PublisherSignalTimeline
      overview={overview}
      detail={overview.initialDetail}
      loadState="ready"
      detailState="ready"
      errorMessage={null}
      detailError={null}
      selectedAlbumId={null}
      queueBusy={false}
      queueMessage={null}
      onSelectPublisher={onSelectPublisher}
      onSelectAlbum={onSelectAlbum}
      onExplore={vi.fn()}
      onPlayPublisher={vi.fn()}
      onRetry={vi.fn()}
      onRetryDetail={vi.fn()}
    />);

    fireEvent.click(screen.getByRole("tab", { name: "Original-year activity" }));
    expect(screen.getByRole("tab", { name: "Original-year activity" })).toHaveAttribute("aria-selected", "true");

    fireEvent.click(screen.getByRole("button", { name: /Parlophone, 2 albums/ }));
    expect(onSelectPublisher).toHaveBeenCalledWith(overview.publishers[0]);

    fireEvent.click(screen.getByRole("button", { name: /1960s Revolver/ }));
    expect(onSelectAlbum).toHaveBeenCalledWith(overview.initialDetail.albums[0]);
  });
});
