import { describe, expect, it } from "vitest";
import {
  mergeRefreshedExplorerPage,
  refreshedExplorerCursor,
  resolveExplorerRefreshPreservation,
  shouldReuseExplorerPage,
} from "./explorerRefresh";

describe("mergeRefreshedExplorerPage", () => {
  it("updates loaded items without moving the user's current position", () => {
    const current = [
      { id: "album-1", title: "First" },
      { id: "album-2", title: "Second" },
      { id: "album-3", title: "Third" },
    ];
    const refreshed = [
      { id: "album-new", title: "Imported" },
      { id: "album-2", title: "Second (retagged)" },
      { id: "album-1", title: "First" },
    ];

    expect(mergeRefreshedExplorerPage(current, refreshed)).toEqual([
      { id: "album-1", title: "First" },
      { id: "album-2", title: "Second (retagged)" },
      { id: "album-3", title: "Third" },
      { id: "album-new", title: "Imported" },
    ]);
  });

  it("keeps the continuation point when a background refresh only reloads the first page", () => {
    const currentCursor = { value: "album-artist-asc:Dee Snider", id: "album-100" };
    const refreshedCursor = { value: "album-artist-asc:Animal Collective", id: "album-50" };

    expect(refreshedExplorerCursor(100, 50, currentCursor, refreshedCursor)).toBe(currentCursor);
    expect(refreshedExplorerCursor(50, 50, currentCursor, refreshedCursor)).toBe(refreshedCursor);
  });

  it("reuses an already loaded page after navigation without blocking explicit refreshes", () => {
    const requestKey = '["albums",{"sort":"yearDesc"},0]';

    expect(shouldReuseExplorerPage(requestKey, requestKey, false)).toBe(true);
    expect(shouldReuseExplorerPage(requestKey, requestKey, true)).toBe(false);
    expect(shouldReuseExplorerPage(requestKey, '["albums",{"sort":"yearDesc"},1]', false)).toBe(false);
  });

  it("keeps refresh preservation pending until the Explorer is visible again", () => {
    const whileRatingsIsVisible = resolveExplorerRefreshPreservation(true, false, true);

    expect(whileRatingsIsVisible).toEqual({
      preservingCurrentView: false,
      pending: true,
    });
    expect(resolveExplorerRefreshPreservation(whileRatingsIsVisible.pending, true, true)).toEqual({
      preservingCurrentView: true,
      pending: false,
    });
  });
});

describe("search replacement", () => {
  const previous = JSON.stringify(["albums", { query: "love=1 AND cr=99 NOT genre:scores OR soundtrack" }, 0]);
  const artist = JSON.stringify(["albums", { query: 'aartist:"Bunny X"' }, 0]);

  it("blocks pagination with the previous cursor while the artist search is pending", () => {
    expect(shouldReuseExplorerPage(previous, artist, false)).toBe(false);
    expect(shouldReuseExplorerPage(null, artist, false)).toBe(false);
    expect(shouldReuseExplorerPage(artist, artist, false)).toBe(true);
  });

  it("consumes a background refresh without merging a different search", () => {
    expect(resolveExplorerRefreshPreservation(true, true, previous === artist)).toEqual({
      preservingCurrentView: false, pending: false,
    });
    expect(resolveExplorerRefreshPreservation(true, true, true)).toEqual({
      preservingCurrentView: true, pending: false,
    });
  });
});
