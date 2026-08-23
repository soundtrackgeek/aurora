import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Artist, Track } from "../../library";
import {
  DeepExplorer,
  type DeepExplorerProps,
  type ExplorerAlbum,
  type ExplorerFilters,
} from "./DeepExplorer";

const tracks: Track[] = [
  {
    id: "track-1",
    trackKey: "file:track-1",
    albumId: "album-1",
    title: "Signal One",
    artist: "Aurora Lines",
    album: "Night Geometry",
    originalYear: 1985,
    releaseYear: null,
    rating: 5,
    loved: true,
    loveState: "loved",
    tagSyncState: null,
    canUndoTagEdit: false,
    durationSeconds: 243,
    genre: "Synthwave",
    playCount: 84,
  },
  {
    id: "track-2",
    trackKey: "file:track-2",
    albumId: "album-1",
    title: "Second Light",
    artist: "Aurora Lines",
    album: "Night Geometry",
    originalYear: 2023,
    releaseYear: 2024,
    rating: null,
    loved: false,
    loveState: "neutral",
    tagSyncState: null,
    canUndoTagEdit: false,
    durationSeconds: 198,
    genre: "Synthwave",
    playCount: null,
  },
];

const artists: Artist[] = [
  { id: "artist-1", name: "Aurora Lines", trackCount: 28, albumCount: 3, playCount: 512 },
];

const albums: ExplorerAlbum[] = [
  {
    id: "album-1",
    title: "Night Geometry",
    artist: "Aurora Lines",
    originalYear: 1985,
    releaseYear: 2024,
    rating: 4.5,
    totalTracks: 12,
    durationSeconds: 2_844,
    genre: "Synthwave",
    lovedTracks: 4,
    ratedTracks: 9,
    albumScore: 412.4,
  },
];

const filters: ExplorerFilters = {
  query: "",
  rating: "all",
  love: "all",
  yearFrom: null,
  yearTo: null,
  yearBasis: "original",
  yearMissing: false,
  genre: null,
  artist: null,
  sort: "newest",
};

afterEach(cleanup);

function explorerProps(overrides: Partial<DeepExplorerProps> = {}): DeepExplorerProps {
  return {
    view: "tracks",
    filters,
    tracks,
    albums,
    artists,
    selectedTrackId: null,
    selectedAlbumId: null,
    selectedArtistId: null,
    albumTracks: [],
    loadState: "ready",
    pageInfo: { loaded: 2, hasMore: true, isLoadingMore: false },
    onViewChange: vi.fn(),
    onFiltersChange: vi.fn(),
    onSelectTrack: vi.fn(),
    onSelectAlbum: vi.fn(),
    onSelectArtist: vi.fn(),
    ...overrides,
  };
}

describe("DeepExplorer", () => {
  it.each(["tracks", "albums", "artists"] as const)("keeps only Sort and Reset in the %s filter bar", (view) => {
    const onClearFilters = vi.fn();
    render(<DeepExplorer {...explorerProps({ view, onClearFilters })} />);

    const filterBar = screen.getByLabelText("Explorer filters");
    expect(filterBar).toHaveTextContent("Sort");
    expect(filterBar).toHaveTextContent("Reset");
    expect(filterBar.querySelectorAll("select")).toHaveLength(1);
    expect(filterBar.querySelectorAll("input")).toHaveLength(0);
    fireEvent.click(screen.getByRole("button", { name: "Reset" }));
    expect(onClearFilters).toHaveBeenCalledOnce();
  });

  it("renders Year without substituting Release Year", () => {
    render(<DeepExplorer {...explorerProps()} />);

    expect(screen.getByRole("row", { name: /Signal One/ })).toHaveTextContent("1985");
    expect(screen.getByRole("row", { name: /Second Light/ })).toHaveTextContent("2023");
  });

  it("toggles chronological and alphabetical directions when a sort is selected again", () => {
    const onViewChange = vi.fn();
    const onFiltersChange = vi.fn();
    const { rerender } = render(<DeepExplorer {...explorerProps({ onViewChange, onFiltersChange })} />);

    fireEvent.click(screen.getByRole("tab", { name: "Albums" }));
    expect(onViewChange).toHaveBeenCalledWith("albums");

    fireEvent.change(screen.getByLabelText("Sort"), { target: { value: "year" } });
    expect(onFiltersChange).toHaveBeenLastCalledWith({ ...filters, sort: "yearDesc" });

    rerender(<DeepExplorer {...explorerProps({ filters: { ...filters, sort: "yearDesc" }, onViewChange, onFiltersChange })} />);
    expect(screen.getByRole("option", { name: "Year · newest" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Sort"), { target: { value: "year" } });
    expect(onFiltersChange).toHaveBeenLastCalledWith({ ...filters, sort: "yearAsc" });

    rerender(<DeepExplorer {...explorerProps({ filters: { ...filters, sort: "titleAsc" }, onViewChange, onFiltersChange })} />);
    expect(screen.getByRole("option", { name: "Title · A–Z" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Sort"), { target: { value: "title" } });
    expect(onFiltersChange).toHaveBeenLastCalledWith({ ...filters, sort: "titleDesc" });
  });

  it("supports arrow-key selection and Enter activation in track rows", () => {
    const onSelectTrack = vi.fn();
    const onActivateTrack = vi.fn();
    render(<DeepExplorer {...explorerProps({ onSelectTrack, onActivateTrack })} />);

    const firstRow = screen.getByRole("row", { name: /Signal One/ });
    const secondRow = screen.getByRole("row", { name: /Second Light/ });
    firstRow.focus();
    fireEvent.keyDown(firstRow, { key: "ArrowDown" });
    expect(onSelectTrack).toHaveBeenLastCalledWith(tracks[1]);
    expect(secondRow).toHaveFocus();

    fireEvent.keyDown(secondRow, { key: "Enter" });
    expect(onActivateTrack).toHaveBeenCalledWith(tracks[1]);
  });

  it("renders an album-id cover surface and controlled album detail", () => {
    const onSelectAlbum = vi.fn();
    const onSelectTrack = vi.fn();
    render(
      <DeepExplorer
        {...explorerProps({
          view: "albums",
          selectedAlbumId: "album-1",
          albumTracks: tracks,
          onSelectAlbum,
          onSelectTrack,
          pageInfo: { loaded: 1, hasMore: false, isLoadingMore: false },
        })}
      />,
    );

    expect(screen.getAllByLabelText("Night Geometry cover")).toHaveLength(2);
    expect(screen.getByRole("complementary", { name: "Night Geometry album details" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close album details" }));
    expect(onSelectAlbum).toHaveBeenCalledWith(null);
  });

  it("only shows Album Score at full completion", () => {
    const completeAlbum = { ...albums[0], ratedTracks: albums[0].totalTracks };
    render(
      <DeepExplorer
        {...explorerProps({
          view: "albums",
          filters: { ...filters, rating: 4.5, sort: "ratingDesc" },
          albums: [completeAlbum],
          selectedAlbumId: completeAlbum.id,
          pageInfo: { loaded: 1, hasMore: false, isLoadingMore: false },
        })}
      />,
    );

    expect(screen.getByText("Album Score 412.4")).toBeInTheDocument();
  });

  it("exposes bounded loading, error, empty, and load-more states", () => {
    const onLoadMore = vi.fn();
    const onRetry = vi.fn();
    const { rerender } = render(<DeepExplorer {...explorerProps({ onLoadMore })} />);

    fireEvent.click(screen.getByRole("button", { name: "Load 50 more" }));
    expect(onLoadMore).toHaveBeenCalledOnce();

    rerender(<DeepExplorer {...explorerProps({ loadState: "error", errorMessage: "Database busy", onRetry })} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Database busy");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(onRetry).toHaveBeenCalledOnce();

    rerender(<DeepExplorer {...explorerProps({ loadState: "loading" })} />);
    expect(screen.getByRole("status")).toHaveTextContent("Opening the deep catalog");

    rerender(<DeepExplorer {...explorerProps({ tracks: [], pageInfo: { loaded: 0, hasMore: false, isLoadingMore: false } })} />);
    expect(screen.getByText("No matches in this orbit")).toBeInTheDocument();
  });
});
