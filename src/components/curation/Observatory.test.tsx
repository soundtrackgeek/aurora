import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ArtistReviewItem } from "../../musicbrainz";
import { Observatory } from "./Observatory";

const item: ArtistReviewItem = {
  artistKey: "m83",
  displayArtist: "M83",
  matchState: "unconfirmed",
  identity: null,
  candidates: [{
    mbid: "6d7b7cd4-254b-4c25-83f6-dd20f98ceacd",
    canonicalName: "M83",
    matchMethod: "catalog-import",
    confidence: null,
    provenance: "catalogImport",
    cacheNameCount: 1,
    verifiedSource: false,
  }],
  decision: null,
  hasExternalConflict: false,
};

const baseProps = {
  items: [item],
  selectedArtistKey: null,
  filter: "needsReview" as const,
  loadState: "ready" as const,
  errorMessage: null,
  hasMore: true,
  loadingMore: false,
  actionBusy: null,
  message: null,
  onFilterChange: vi.fn(),
  onSelect: vi.fn(),
  onLoadMore: vi.fn(),
  onRefresh: vi.fn(),
  onUndo: vi.fn(),
  onExport: vi.fn(),
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Observatory", () => {
  it("selects artists, filters, pages, undoes, and exports", () => {
    render(<Observatory {...baseProps} />);
    fireEvent.click(screen.getByRole("button", { name: /M83/ }));
    expect(baseProps.onSelect).toHaveBeenCalledWith(item);
    fireEvent.click(screen.getByRole("tab", { name: "Conflicts" }));
    expect(baseProps.onFilterChange).toHaveBeenCalledWith("conflict");
    fireEvent.click(screen.getByRole("button", { name: "Load next review page" }));
    expect(baseProps.onLoadMore).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Undo last" }));
    expect(baseProps.onUndo).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Export overlay snapshot" }));
    expect(baseProps.onExport).toHaveBeenCalledOnce();
  });

  it("renders loading, empty, and recoverable error states honestly", () => {
    const { rerender } = render(<Observatory {...baseProps} items={[]} loadState="loading" hasMore={false} />);
    expect(screen.getByRole("status")).toHaveTextContent("bounded review page");
    rerender(<Observatory {...baseProps} items={[]} loadState="error" errorMessage="Cache busy" hasMore={false} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Cache busy");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(baseProps.onRefresh).toHaveBeenCalledOnce();
    rerender(<Observatory {...baseProps} items={[]} loadState="ready" hasMore={false} />);
    expect(screen.getByText("No artists in this review slice")).toBeInTheDocument();
  });
});
