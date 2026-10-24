import { describe, expect, it } from "vitest";
import { loadInboxSnapshot } from "./inbox";
import { mergeRefreshedInboxAlbums } from "./inboxRefresh";

describe("Inbox refresh", () => {
  it("updates metadata in place, removes departed albums and appends new arrivals in scan order", async () => {
    const base = (await loadInboxSnapshot()).albums[0];
    const current = ["first", "departed", "edited"].map((id) => ({ ...base, id }));
    const edited = { ...current[2], album: "Corrected album", modifiedAtMs: base.modifiedAtMs + 1_000 };
    const arrivals = ["newest", "older arrival"].map((id) => ({ ...base, id }));
    const refreshed = [edited, arrivals[0], current[0], arrivals[1]];
    const merged = mergeRefreshedInboxAlbums(current, refreshed);
    expect(merged.map((album) => album.id)).toEqual(["first", "edited", "newest", "older arrival"]);
    expect(merged[1]).toBe(edited);
    expect(mergeRefreshedInboxAlbums([], refreshed)).toEqual(refreshed);
    expect(mergeRefreshedInboxAlbums(current, [])).toEqual([]);
  });

  it("keeps an explicitly renamed album in its old slot without retaining stale paths or duplicating it", async () => {
    const base = (await loadInboxSnapshot()).albums[0];
    const current = ["first", "renamed", "last"].map((id) => ({ ...base, id }));
    const renamed = { ...base, id: "new-path-id", path: "C:\\Inbox\\Corrected album" };
    const merged = mergeRefreshedInboxAlbums(current, [renamed, current[2], current[0]], { from: "renamed", to: renamed.id });
    expect(merged.map((album) => album.id)).toEqual(["first", "new-path-id", "last"]);
    expect(merged[1]).toBe(renamed);
  });
});
