import { describe, expect, it } from "vitest";
import { chartAlbumSearchQuery } from "./charts";
import { filterTracks, type Track } from "./library";

function track(id: string, album: string, artist = "Huey Lewis and the News"): Track {
  return {
    id, trackKey: id, albumId: id, title: "A song", album, artist,
    releaseYear: 1983, rating: null, loved: false, loveState: "neutral",
    tagSyncState: null, canUndoTagEdit: false, durationSeconds: null, genre: null, playCount: null,
  };
}

describe("chart album library search", () => {
  it("finds every exact Sports album without artist or longer-title matches", () => {
    const tracks = [
      track("sports", "Sports"),
      track("other-sports", "Sports", "Another artist"),
      track("artist", "People Can't Stop Chillin", "Sports"),
      track("longer", "Sports Illustrated"),
    ];
    const query = chartAlbumSearchQuery({ title: "Sports", matchedAlbumTitle: "Sports" });
    expect(query).toBe('album:"Sports"');
    expect(filterTracks(tracks, query, null).map((item) => item.id)).toEqual(["sports", "other-sports"]);
  });

  it("uses the matched catalog title when the chart spelling differs", () => {
    const query = chartAlbumSearchQuery({ title: "Sports", matchedAlbumTitle: "Sports (Expanded Edition)" });
    expect(filterTracks([track("expanded", "Sports (Expanded Edition)"), track("original", "Sports")], query, null).map((item) => item.id)).toEqual(["expanded"]);
  });

  it.each([null, "", "   "])("falls back to the exact chart title when the catalog title is %j", (matchedAlbumTitle) => {
    expect(chartAlbumSearchQuery({ title: "Sports", matchedAlbumTitle })).toBe('album:"Sports"');
  });

  it("treats embedded quotes and query operators as literal album text", () => {
    const title = 'The "Sports", AND OR NOT Collection';
    const query = chartAlbumSearchQuery({ title: "Sports", matchedAlbumTitle: title });
    expect(filterTracks([track("quoted", title), track("other", "Sports")], query, null).map((item) => item.id)).toEqual(["quoted"]);
  });
});
