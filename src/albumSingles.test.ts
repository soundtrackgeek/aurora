import { describe, expect, it } from "vitest";

import { applyAlbumPopularity, filterTracks, officialSingleLabel, type Track } from "./library";

const track = (trackKey: string): Track => ({ trackKey, title: trackKey, artist: "Bob Seger" }) as Track;

describe("official singles on album tracks", () => {
  it("attaches the single to the matching track and clears the others", () => {
    const tracks = applyAlbumPopularity(
      [track("a"), track("b")],
      { tracks: [{ trackKey: "b", rank: 1 }], singles: [{ trackKey: "a", title: "Take a Chance", year: 1991 }] },
    );
    expect(tracks[0].officialSingle).toEqual({ title: "Take a Chance", year: 1991 });
    expect(tracks[0].lastFmAlbumRank).toBeNull();
    expect(tracks[1].officialSingle).toBeNull();
    expect(tracks[1].lastFmAlbumRank).toBe(1);
  });

  it("keeps working with a payload from before singles existed", () => {
    const [only] = applyAlbumPopularity([track("a")], { tracks: [] });
    expect(only.officialSingle).toBeNull();
  });

  it("drops a single when a later refresh no longer reports it", () => {
    const first = applyAlbumPopularity([track("a")], { tracks: [], singles: [{ trackKey: "a", title: "A", year: null }] });
    const second = applyAlbumPopularity(first, { tracks: [], singles: [] });
    expect(second[0].officialSingle).toBeNull();
  });

  it("words the tooltip with or without a year", () => {
    expect(officialSingleLabel({ title: "Take a Chance", year: 1991 })).toBe("Official single: Take a Chance (1991)");
    expect(officialSingleLabel({ title: "Take a Chance", year: null })).toBe("Official single: Take a Chance");
  });
});

describe("single: search in local filtering", () => {
  const single = { title: "Hit", year: 1991 };
  const tracks = [
    { ...track("a"), id: "a", albumId: "x", officialSingle: single },
    { ...track("b"), id: "b", albumId: "x", officialSingle: null },
    { ...track("c"), id: "c", albumId: "y" },
  ] as Track[];
  const ids = (query: string) => filterTracks(tracks, query, null).map((item) => item.id);

  it("treats a song as a single or not, like the desktop Songs view", () => {
    expect(ids("single:1")).toEqual(["a"]);
    expect(ids("single:0")).toEqual(["b", "c"]);
    expect(ids("single:1..3")).toEqual(["a"]);
    expect(ids("single:..0")).toEqual(["b", "c"]);
    expect(ids("NOT single:1")).toEqual(["b", "c"]);
  });

  it("accepts singles: as an alias and rejects counts the shorthand does not cover", () => {
    expect(ids("singles:1")).toEqual(["a"]);
    expect(() => filterTracks(tracks, "single:2", null)).toThrow("single must be 0, 1, or an inclusive range such as single:1..3.");
    expect(() => filterTracks(tracks, "single:5..2", null)).toThrow("single range must start at or before its ending number.");
  });
});
