import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Artist, Track } from "../../library";
import {
  DeepExplorer,
  type DeepExplorerProps,
  type ExplorerAlbum,
  type ExplorerFilters,
} from "./DeepExplorer";
import { resolveExplorerAlbumInspectorContext } from "./inspectorContext";

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
    trackNumber: 1,
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
    trackNumber: 2,
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
    publisher: "EMI Records",
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
  it("keeps album, track, and artist inspector context on the selected album", () => {
    const staleTrack: Track = {
      ...tracks[0],
      id: "stale-track",
      trackKey: "file:stale-track",
      albumId: "another-album",
      artist: "Another Artist",
      album: "Another Album",
    };

    expect(resolveExplorerAlbumInspectorContext(albums, "album-1", tracks, staleTrack)).toEqual({
      album: albums[0],
      track: tracks[0],
      artistName: "Aurora Lines",
    });
    expect(resolveExplorerAlbumInspectorContext(albums, "album-1", [], staleTrack)).toEqual({
      album: albums[0],
      track: null,
      artistName: "Aurora Lines",
    });
  });

  it("uses the selected track artist for the Artist inspector", () => {
    const soundtrackTrack: Track = {
      ...tracks[1],
      artist: "Mark Mancina",
      displayArtist: "Billy Idol",
      album: "Speed",
    };

    expect(resolveExplorerAlbumInspectorContext(
      albums,
      "album-1",
      [tracks[0], soundtrackTrack],
      soundtrackTrack,
    )).toEqual({
      album: albums[0],
      track: soundtrackTrack,
      artistName: "Billy Idol",
    });
  });

  it.each(["tracks", "albums", "artists"] as const)("keeps only Sort and Reset in the %s filter bar", (view) => {
    const onClearFilters = vi.fn();
    render(<DeepExplorer {...explorerProps({ view, onClearFilters })} />);

    const filterBar = screen.getByLabelText("Explorer filters");
    expect(filterBar).toHaveTextContent("Sort");
    expect(filterBar).toHaveTextContent("Reset");
    expect(filterBar.querySelectorAll("select")).toHaveLength(0);
    expect(filterBar.querySelectorAll("input")).toHaveLength(0);
    expect(screen.getByRole("button", { name: /^Sort:/ })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Reset" }));
    expect(onClearFilters).toHaveBeenCalledOnce();
  });

  it("renders Year without substituting Release Year", () => {
    render(<DeepExplorer {...explorerProps()} />);

    expect(screen.getByRole("row", { name: /Signal One/ })).toHaveTextContent("1985");
    expect(screen.getByRole("row", { name: /Second Light/ })).toHaveTextContent("2023");
  });

  it("keeps the active sort clickable after another choice is hovered, then reverses it", () => {
    const onViewChange = vi.fn();
    const onFiltersChange = vi.fn();
    const { rerender } = render(<DeepExplorer {...explorerProps({ onViewChange, onFiltersChange })} />);

    fireEvent.click(screen.getByRole("tab", { name: "Albums" }));
    expect(onViewChange).toHaveBeenCalledWith("albums");

    fireEvent.click(screen.getByRole("button", { name: "Sort: Added · newest" }));
    fireEvent.click(screen.getByRole("menuitemradio", { name: "Artist · A–Z" }));
    expect(onFiltersChange).toHaveBeenLastCalledWith({ ...filters, sort: "artistAsc" });

    rerender(<DeepExplorer {...explorerProps({ filters: { ...filters, sort: "artistAsc" }, onViewChange, onFiltersChange })} />);
    fireEvent.click(screen.getByRole("button", { name: "Sort: Artist · A–Z" }));
    fireEvent.mouseEnter(screen.getByRole("menuitemradio", { name: "Title · A–Z" }));
    const activeArtistSort = screen.getByRole("menuitemradio", { name: "Artist · A–Z" });
    expect(activeArtistSort).toBeEnabled();
    fireEvent.click(activeArtistSort);
    expect(onFiltersChange).toHaveBeenLastCalledWith({ ...filters, sort: "artistDesc" });

    rerender(<DeepExplorer {...explorerProps({ filters: { ...filters, sort: "yearDesc" }, onViewChange, onFiltersChange })} />);
    fireEvent.click(screen.getByRole("button", { name: "Sort: Year · newest" }));
    fireEvent.click(screen.getByRole("menuitemradio", { name: "Year · newest" }));
    expect(onFiltersChange).toHaveBeenLastCalledWith({ ...filters, sort: "yearAsc" });
  });

  it("offers added order for albums and reverses it to oldest", () => {
    const onFiltersChange = vi.fn();
    render(<DeepExplorer {...explorerProps({ view: "albums", onFiltersChange })} />);

    fireEvent.click(screen.getByRole("button", { name: "Sort: Added · newest" }));
    expect(screen.getByRole("menuitemradio", { name: "Added · newest" })).toBeVisible();
    fireEvent.click(screen.getByRole("menuitemradio", { name: "Added · newest" }));

    expect(onFiltersChange).toHaveBeenLastCalledWith({ ...filters, sort: "oldest" });
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

  it("uses Windows Ctrl and Shift selection in the main track list", () => {
    const onSelectionChange = vi.fn();
    render(<DeepExplorer {...explorerProps({ onSelectionChange })} />);

    const firstRow = screen.getByRole("row", { name: /Signal One/ });
    const secondRow = screen.getByRole("row", { name: /Second Light/ });
    fireEvent.click(firstRow);
    fireEvent.click(secondRow, { shiftKey: true });
    expect(firstRow).toHaveAttribute("aria-selected", "true");
    expect(secondRow).toHaveAttribute("aria-selected", "true");
    expect(onSelectionChange).toHaveBeenLastCalledWith({ kind: "tracks", tracks });

    fireEvent.click(firstRow, { ctrlKey: true });
    expect(firstRow).toHaveAttribute("aria-selected", "false");
    expect(secondRow).toHaveAttribute("aria-selected", "true");
    expect(onSelectionChange).toHaveBeenLastCalledWith({ kind: "tracks", tracks: [tracks[1]] });
  });

  it("renders album detail under the selected cover row without deselecting a plain-clicked album", () => {
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
    const albumButton = screen.getByRole("button", { expanded: true });
    const albumRow = albumButton.closest(".deep-explorer-album-row");
    expect(albumRow).not.toBeNull();
    expect(within(albumRow as HTMLElement).getByRole("complementary", { name: "Night Geometry album details" })).toBeInTheDocument();
    fireEvent.click(albumButton);
    expect(onSelectAlbum).toHaveBeenCalledWith(albums[0]);
    fireEvent.click(screen.getByRole("button", { name: "Close album details" }));
    expect(onSelectAlbum).toHaveBeenCalledWith(null);
  });

  it("uses Windows Ctrl and Shift selection in the album grid", () => {
    const extraAlbums = [
      ...albums,
      { ...albums[0], id: "album-2", title: "Electric Dawn" },
      { ...albums[0], id: "album-3", title: "Violet Static" },
    ];
    const onSelectionChange = vi.fn();
    render(<DeepExplorer {...explorerProps({
      view: "albums",
      albums: extraAlbums,
      onSelectionChange,
      pageInfo: { loaded: 3, hasMore: false, isLoadingMore: false },
    })} />);

    const first = screen.getByRole("button", { name: /Night Geometry/ });
    const third = screen.getByRole("button", { name: /Violet Static/ });
    fireEvent.click(first);
    fireEvent.click(third, { shiftKey: true });
    expect(first).toHaveAttribute("aria-pressed", "true");
    expect(third).toHaveAttribute("aria-pressed", "true");
    expect(onSelectionChange).toHaveBeenLastCalledWith({ kind: "albums", albums: extraAlbums });

    fireEvent.click(first, { ctrlKey: true });
    expect(first).toHaveAttribute("aria-pressed", "false");
    expect(onSelectionChange).toHaveBeenLastCalledWith({ kind: "albums", albums: extraAlbums.slice(1) });
  });

  it("shows each track's display artist beside its title in album detail", () => {
    const soundtrackTrack: Track = {
      ...tracks[0],
      artist: "Various Artists",
      displayArtist: "Andrea & Hot Mink",
    };
    render(
      <DeepExplorer
        {...explorerProps({
          view: "albums",
          selectedAlbumId: albums[0].id,
          albumTracks: [soundtrackTrack],
          pageInfo: { loaded: 1, hasMore: false, isLoadingMore: false },
        })}
      />,
    );

    const albumDetail = screen.getByRole("complementary", { name: "Night Geometry album details" });
    expect(within(albumDetail).getByText("[Andrea & Hot Mink]")).toBeVisible();
    expect(within(albumDetail).queryByText("[Various Artists]")).not.toBeInTheDocument();
  });

  it("uses track numbers instead of the repeated year in album detail", () => {
    render(
      <DeepExplorer
        {...explorerProps({
          view: "albums",
          selectedAlbumId: albums[0].id,
          albumTracks: tracks,
          pageInfo: { loaded: 1, hasMore: false, isLoadingMore: false },
        })}
      />,
    );

    const albumTracks = within(screen.getByRole("grid", { name: "Album tracks" }));
    expect(albumTracks.getByRole("columnheader", { name: "Track" })).toBeVisible();
    expect(albumTracks.queryByRole("columnheader", { name: "Year" })).not.toBeInTheDocument();
    expect(albumTracks.getByText("01")).toBeVisible();
    expect(albumTracks.getByText("02")).toBeVisible();
  });

  it("shows half-star Album Rating and Album Score together on album cards and detail", () => {
    render(
      <DeepExplorer
        {...explorerProps({
          view: "albums",
          filters: { ...filters, rating: 4.5, sort: "ratingDesc" },
          selectedAlbumId: albums[0].id,
          pageInfo: { loaded: 1, hasMore: false, isLoadingMore: false },
        })}
      />,
    );

    expect(screen.getAllByLabelText("Album rating 4.5 out of 5 stars")).toHaveLength(2);
    expect(document.querySelectorAll(".deep-explorer-album-rating__star.is-half")).toHaveLength(2);
    expect(screen.getByText("Score 412.4")).toBeInTheDocument();
    expect(screen.getByText("Album Score 412.4")).toBeInTheDocument();
    expect(screen.getByText((_, element) => (
      element?.classList.contains("deep-explorer-album__metadata") === true
      && element.textContent?.replace(/\s+/g, " ").trim() === "1985 — Synthwave — EMI Records"
    ))).toBeVisible();
    expect(screen.getByText((_, element) => (
      element?.classList.contains("deep-explorer-album__length") === true
      && element.textContent?.replace(/\s+/g, " ").trim() === "12 tracks — 47:24"
    ))).toBeVisible();
    const albumDetail = screen.getByRole("complementary", { name: "Night Geometry album details" });
    expect(within(albumDetail).getByText((_, element) => (
      element?.classList.contains("deep-explorer-album-publisher") === true
      && element.textContent?.replace(/\s+/g, " ").trim() === "Synthwave — EMI Records"
    ))).toBeVisible();
  });

  it("requires confirmation before deleting an album track", async () => {
    const onDeleteTracks = vi.fn().mockResolvedValue(undefined);
    render(
      <DeepExplorer
        {...explorerProps({
          view: "albums",
          selectedAlbumId: albums[0].id,
          albumTracks: tracks,
          onDeleteTracks,
          pageInfo: { loaded: 1, hasMore: false, isLoadingMore: false },
        })}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Delete Signal One" }));
    const dialog = screen.getByRole("alertdialog", { name: "Delete “Signal One”?" });
    expect(dialog).toHaveTextContent("permanently deletes the MP3 from disk");
    expect(dialog).toHaveTextContent("record one deleted track in Updates");
    expect(onDeleteTracks).not.toHaveBeenCalled();

    fireEvent.click(within(dialog).getByRole("button", { name: "Delete track" }));
    await waitFor(() => expect(onDeleteTracks).toHaveBeenCalledWith([tracks[0]]));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("supports Ctrl toggles and Shift ranges for bulk album-track deletion", async () => {
    const onDeleteTracks = vi.fn().mockResolvedValue(undefined);
    render(
      <DeepExplorer
        {...explorerProps({
          view: "albums",
          selectedAlbumId: albums[0].id,
          albumTracks: tracks,
          onDeleteTracks,
          pageInfo: { loaded: 1, hasMore: false, isLoadingMore: false },
        })}
      />,
    );

    const firstRow = screen.getByRole("row", { name: /Signal One/ });
    const secondRow = screen.getByRole("row", { name: /Second Light/ });
    fireEvent.click(firstRow);
    fireEvent.click(secondRow, { shiftKey: true });
    expect(firstRow).toHaveAttribute("aria-selected", "true");
    expect(secondRow).toHaveAttribute("aria-selected", "true");
    fireEvent.click(secondRow, { ctrlKey: true });
    expect(secondRow).toHaveAttribute("aria-selected", "false");
    fireEvent.click(secondRow, { ctrlKey: true });
    expect(firstRow).toHaveAttribute("aria-selected", "true");
    expect(secondRow).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("2 tracks selected")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Delete selected" }));
    const dialog = screen.getByRole("alertdialog", { name: "Delete 2 selected tracks?" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete 2 tracks" }));
    await waitFor(() => expect(onDeleteTracks).toHaveBeenCalledWith(tracks));
  });

  it("marks the current playback track independently from the selected row", () => {
    render(
      <DeepExplorer
        {...explorerProps({
          view: "albums",
          selectedAlbumId: albums[0].id,
          selectedTrackId: tracks[0].id,
          currentTrackKey: tracks[1].trackKey,
          playbackActive: true,
          albumTracks: tracks,
          pageInfo: { loaded: 1, hasMore: false, isLoadingMore: false },
        })}
      />,
    );

    const selectedRow = screen.getByRole("row", { name: /Signal One/ });
    const playingRow = screen.getByRole("row", { name: /Second Light/ });
    expect(selectedRow).toHaveAttribute("aria-selected", "true");
    expect(selectedRow).not.toHaveAttribute("aria-current");
    expect(playingRow).toHaveAttribute("aria-current", "true");
    expect(playingRow).toHaveTextContent("Currently playing");
  });

  it("loads the next bounded page when the scroll sentinel enters view", () => {
    const onLoadMore = vi.fn();
    const onRetry = vi.fn();
    let intersect: (entries: IntersectionObserverEntry[]) => void = () => undefined;
    const observe = vi.fn();
    const disconnect = vi.fn();
    vi.stubGlobal("IntersectionObserver", vi.fn(function (callback: IntersectionObserverCallback) {
      intersect = (entries) => callback(entries, {} as IntersectionObserver);
      return { observe, disconnect, unobserve: vi.fn(), takeRecords: () => [], root: null, rootMargin: "", thresholds: [] };
    }));
    const { rerender } = render(<DeepExplorer {...explorerProps({ onLoadMore })} />);

    expect(screen.queryByRole("button", { name: "Load 50 more" })).not.toBeInTheDocument();
    expect(screen.getByText("Scroll for the next 50")).toBeInTheDocument();
    expect(observe).toHaveBeenCalledOnce();
    intersect([{ isIntersecting: true } as IntersectionObserverEntry]);
    expect(onLoadMore).toHaveBeenCalledOnce();

    intersect([{ isIntersecting: true } as IntersectionObserverEntry]);
    expect(onLoadMore).toHaveBeenCalledOnce();

    rerender(<DeepExplorer {...explorerProps({
      onLoadMore,
      filters: { ...filters, query: "new result set" },
    })} />);
    intersect([{ isIntersecting: true } as IntersectionObserverEntry]);
    expect(onLoadMore).toHaveBeenCalledTimes(2);

    rerender(<DeepExplorer {...explorerProps({ loadState: "error", errorMessage: "Database busy", onRetry })} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Database busy");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(onRetry).toHaveBeenCalledOnce();

    rerender(<DeepExplorer {...explorerProps({ loadState: "loading" })} />);
    expect(screen.getByRole("status")).toHaveTextContent("Opening the deep catalog");

    rerender(<DeepExplorer {...explorerProps({ tracks: [], pageInfo: { loaded: 0, hasMore: false, isLoadingMore: false } })} />);
    expect(screen.getByText("No matches in this orbit")).toBeInTheDocument();
    vi.unstubAllGlobals();
  });
});
