import { describe, expect, it, vi } from "vitest";
import {
  createExplorerRefreshQueue,
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

describe("background search reconciliation", () => {
  function deferred<T>() {
    let resolve!: (value: T) => void;
    let reject!: (error: Error) => void;
    const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
    return { promise, resolve, reject };
  }

  it("keeps local results usable while a file refresh is pending, then replaces excluded matches", async () => {
    const refresh = createExplorerRefreshQueue<string[]>();
    const network = deferred<string[]>();
    let visible = ["local-match", "now-excluded-by-genre"];
    refresh({ load: () => network.promise, cancelled: () => false,
      apply: (value) => { visible = value; }, failed: () => {} });
    expect(visible).toEqual(["local-match", "now-excluded-by-genre"]);
    network.resolve(["local-match"]);
    await vi.waitFor(() => expect(visible).toEqual(["local-match"]));
  });

  it("serializes refreshes and drops obsolete searches and pagination windows", async () => {
    const refresh = createExplorerRefreshQueue<number>();
    const first = deferred<number>();
    let request = 1;
    const applied = vi.fn();
    const failed = vi.fn();
    refresh({ load: () => first.promise, cancelled: () => request !== 1, apply: applied, failed });
    request = 2;
    const superseded = vi.fn(async () => 50);
    refresh({ load: superseded, cancelled: () => request !== 2, apply: applied, failed });
    request = 3; // The user loaded another local page before reconciliation finished.
    const latest = vi.fn(async () => 100);
    refresh({ load: latest, cancelled: () => request !== 3, apply: applied, failed });
    expect(latest).not.toHaveBeenCalled();
    first.resolve(1);
    await vi.waitFor(() => expect(applied).toHaveBeenCalledExactlyOnceWith(100));
    expect(superseded).not.toHaveBeenCalled();
    expect(failed).not.toHaveBeenCalled();
  });

  it("retains local results on failure and continues with the next search", async () => {
    const refresh = createExplorerRefreshQueue<string[]>();
    const network = deferred<string[]>();
    let visible = ["local"];
    const failed = vi.fn();
    refresh({ load: () => network.promise, cancelled: () => false,
      apply: (value) => { visible = value; }, failed });
    network.reject(new Error("Share unavailable"));
    await vi.waitFor(() => expect(failed).toHaveBeenCalledOnce());
    expect(visible).toEqual(["local"]);
    refresh({ load: async () => ["next"], cancelled: () => false,
      apply: (value) => { visible = value; }, failed });
    await vi.waitFor(() => expect(visible).toEqual(["next"]));
  });

  it("does not apply or report errors after leaving the search", async () => {
    const refresh = createExplorerRefreshQueue<number>();
    const network = deferred<number>();
    let cancelled = false;
    const apply = vi.fn();
    const failed = vi.fn();
    refresh({ load: () => network.promise, cancelled: () => cancelled, apply, failed });
    cancelled = true;
    network.reject(new Error("Obsolete"));
    await network.promise.catch(() => {});
    expect(apply).not.toHaveBeenCalled();
    expect(failed).not.toHaveBeenCalled();
  });
});
