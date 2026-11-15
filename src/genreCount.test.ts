import { describe, expect, it } from "vitest";
import { filterTracks, type Track } from "./library";

const track = (id: string, albumId: string, genre: string | null): Track => ({ id, trackKey: id, title: id, artist: "A", albumId, genre }) as Track;

describe("genres: search in local filtering", () => {
  const tracks = [
    track("one-1", "one", "Rock"), track("one-2", "one", "rock "),
    track("multi-1", "multi", "Electro; Dance"), track("multi-2", "multi", "Electro"),
    track("mixed-1", "mixed", "Rock"), track("mixed-2", "mixed", "Pop"),
    track("none-1", "none", null),
  ];
  const albums = (query: string) => [...new Set(filterTracks(tracks, query, null).map((item) => item.albumId))];

  it("counts genre values per album, so genres:1 follows the one-genre rule", () => {
    expect(albums("genres:1")).toEqual(["one"]);
    expect(albums("genres:2..")).toEqual(["multi", "mixed"]);
    expect(albums("genres:0")).toEqual(["none"]);
    expect(albums("NOT genres:1")).toEqual(["multi", "mixed", "none"]);
  });

  it("rejects counts that are not whole numbers", () => {
    expect(() => filterTracks(tracks, "genres:x", null)).toThrow("genres bounds must be non-negative whole genre counts.");
  });
});
