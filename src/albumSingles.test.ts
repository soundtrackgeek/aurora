import { describe, expect, it } from "vitest";

import { applyAlbumPopularity, officialSingleLabel, type Track } from "./library";

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
