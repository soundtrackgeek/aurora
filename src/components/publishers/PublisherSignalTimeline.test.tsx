import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PublisherOverview } from "../../publishers";
import { savePublisherLogoOverride } from "../../publisherLogos";
import { PublisherSignalTimeline } from "./PublisherSignalTimeline";

afterEach(cleanup);
beforeEach(() => window.localStorage.clear());

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
    const { container } = render(<PublisherSignalTimeline
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

    const chart = screen.getByRole("img", { name: /Parlophone, 2 albums/ });
    expect(chart).toHaveAttribute("preserveAspectRatio", "none");
    expect(chart.querySelector(".publisher-signal__line")?.getAttribute("d")).toMatch(/^M151\.58 .+ L445\.26 /);
    expect(Number.parseFloat(container.querySelector<HTMLElement>(".publisher-signal__endpoint")?.style.left ?? "NaN"))
      .toBeCloseTo((1997 - 1950) / (2026 - 1950) * 100);

    fireEvent.click(screen.getByRole("tab", { name: "Original-year activity" }));
    expect(screen.getByRole("tab", { name: "Original-year activity" })).toHaveAttribute("aria-selected", "true");

    fireEvent.click(screen.getByRole("button", { name: /Parlophone, 2 albums/ }));
    expect(onSelectPublisher).toHaveBeenCalledWith(overview.publishers[0]);

    fireEvent.click(screen.getByRole("button", { name: /1960s Revolver/ }));
    expect(onSelectAlbum).toHaveBeenCalledWith(overview.initialDetail.albums[0]);
  });

  it("loads and clears a device-local publisher logo override", () => {
    savePublisherLogoOverride({}, "Parlophone", "data:image/png;base64,aGVsbG8=");
    const { container } = render(<PublisherSignalTimeline
      overview={overview}
      detail={overview.initialDetail}
      loadState="ready"
      detailState="ready"
      errorMessage={null}
      detailError={null}
      selectedAlbumId={null}
      queueBusy={false}
      queueMessage={null}
      onSelectPublisher={vi.fn()}
      onSelectAlbum={vi.fn()}
      onExplore={vi.fn()}
      onPlayPublisher={vi.fn()}
      onRetry={vi.fn()}
      onRetryDetail={vi.fn()}
    />);

    expect(container.querySelectorAll(".publisher-logo.has-image img")).toHaveLength(2);
    fireEvent.click(screen.getByRole("button", { name: "Use monogram" }));
    expect(container.querySelectorAll(".publisher-logo--generated")).toHaveLength(2);
    expect(screen.getByRole("status")).toHaveTextContent("Restored the Aurora monogram for Parlophone.");
  });

  it("rejects an unsafe local logo format inline", async () => {
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
      onSelectPublisher={vi.fn()}
      onSelectAlbum={vi.fn()}
      onExplore={vi.fn()}
      onPlayPublisher={vi.fn()}
      onRetry={vi.fn()}
      onRetryDetail={vi.fn()}
    />);

    fireEvent.change(screen.getByLabelText("Choose a local logo for Parlophone"), {
      target: { files: [new File(["<svg />"], "logo.svg", { type: "image/svg+xml" })] },
    });
    expect(await screen.findByRole("alert")).toHaveTextContent("Choose a PNG, JPEG, or WebP image.");
  });
});
