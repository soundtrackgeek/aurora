import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { YearDetail, YearOverview } from "../../years";
import { YearAlbumInspector, YearsExplorer } from "./YearsExplorer";

afterEach(cleanup);

const album = {
  id: "album-1",
  title: "Blade Runner (Expanded Edition)",
  artist: "Vangelis",
  originCountryCode: "GR",
  originCountryName: "Greece",
  originalYear: 1982,
  releaseYear: 2025,
  totalTracks: 32,
  ratedTracks: 28,
  lovedTracks: 5,
  durationSeconds: 4542,
  genre: "Stage & Screen",
  rating: 5,
  formats: ["MP3"],
  avgBitrateKbps: 320,
};

const detail: YearDetail = {
  selection: { basis: "release", year: 2025 },
  summary: { albumCount: 2_606, trackCount: 59_620, ratedTracks: 24_829, lovedTracks: 234, durationSeconds: 1_000_000 },
  flows: [{ year: 1982, albumCount: 792, trackCount: 12_000 }],
  albums: [album],
};

const overview: YearOverview = {
  originalYears: [
    { year: 1982, albumCount: 413, trackCount: 11_814, ratedTracks: 7_401, lovedTracks: 612 },
    { year: 2025, albumCount: 1_800, trackCount: 42_003, ratedTracks: 18_000, lovedTracks: 180 },
  ],
  releaseYears: [
    { year: 1982, albumCount: 333, trackCount: 3_373, ratedTracks: 2_000, lovedTracks: 100 },
    { year: 2025, albumCount: 2_606, trackCount: 59_620, ratedTracks: 24_829, lovedTracks: 234 },
  ],
  stats: {
    firstYear: 1946,
    lastYear: 2026,
    differentAlbums: 9_385,
    differentTracks: 209_671,
    missingOriginalAlbums: 172,
    missingOriginalTracks: 1_887,
    missingReleaseAlbums: 37_016,
    missingReleaseTracks: 422_009,
  },
  initialDetail: detail,
};

function renderExplorer() {
  const callbacks = {
    onSelect: vi.fn(),
    onSelectAlbum: vi.fn(),
    onExplore: vi.fn(),
    onPlayYear: vi.fn(),
    onPlayAlbum: vi.fn(),
    onRetry: vi.fn(),
    onRetryDetail: vi.fn(),
  };
  render(<YearsExplorer
    overview={overview}
    detail={detail}
    loadState="ready"
    detailState="ready"
    errorMessage={null}
    detailError={null}
    selectedAlbumId={null}
    queueBusy={false}
    queueMessage={null}
    {...callbacks}
  />);
  return callbacks;
}

describe("YearsExplorer", () => {
  it("makes either timeline and landscape mode interactive", () => {
    const callbacks = renderExplorer();
    expect(screen.getByRole("tab", { name: "Two clocks" })).toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("button", { name: /Original Year 1982:/ }));
    expect(callbacks.onSelect).toHaveBeenCalledWith({ basis: "original", year: 1982 });

    fireEvent.click(screen.getByRole("tab", { name: "Original landscape" }));
    expect(callbacks.onSelect).toHaveBeenCalledWith({ basis: "original", year: 2025 });
  });

  it("opens albums and wires the primary Explore and Play actions", () => {
    const callbacks = renderExplorer();
    fireEvent.click(screen.getByRole("button", { name: /Blade Runner/ }));
    expect(callbacks.onSelectAlbum).toHaveBeenCalledWith(album);
    fireEvent.click(screen.getByRole("button", { name: "Explore release 2025" }));
    expect(callbacks.onExplore).toHaveBeenCalledWith(detail.selection);
    fireEvent.click(screen.getByRole("button", { name: "Play this release year" }));
    expect(callbacks.onPlayYear).toHaveBeenCalledWith(detail.selection);
  });

  it("keeps missing Original and Release Year separate", () => {
    const callbacks = renderExplorer();
    fireEvent.click(screen.getByRole("button", { name: /Missing Original Year/ }));
    fireEvent.click(screen.getByRole("button", { name: /Missing Release Year/ }));
    expect(callbacks.onSelect).toHaveBeenNthCalledWith(1, { basis: "original", year: null });
    expect(callbacks.onSelect).toHaveBeenNthCalledWith(2, { basis: "release", year: null });
  });
});

describe("YearAlbumInspector", () => {
  it("shows both dates and plays the selected edition", () => {
    const onPlay = vi.fn();
    render(<YearAlbumInspector album={album} busy={false} onPlay={onPlay} onOpenArtistAlbums={vi.fn()} />);
    expect(screen.getByText("Original Year").nextSibling).toHaveTextContent("1982");
    expect(screen.getByText("Release Year").nextSibling).toHaveTextContent("2025");
    expect(screen.getByText("Format").nextSibling).toHaveTextContent("MP3 · 320 kbps");
    expect(screen.getByRole("img", { name: "Greece origin country" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Play album" }));
    expect(onPlay).toHaveBeenCalledWith(album);
  });

  it("can show album ratings with two decimal places", () => {
    render(<YearAlbumInspector album={{ ...album, rating: 4.33 }} busy={false} onPlay={vi.fn()} onOpenArtistAlbums={vi.fn()} ratingDigits={2} />);

    expect(screen.getByText("Rating").nextSibling).toHaveTextContent("4.33");
  });
});
