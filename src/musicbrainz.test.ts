import { describe, expect, it } from "vitest";
import { loadArtistIntelligence } from "./musicbrainz";

describe("MusicBrainz browser adapter", () => {
  it("provides a truthful enriched preview for M83", async () => {
    const intelligence = await loadArtistIntelligence("M83");
    expect(intelligence.matchState).toBe("unconfirmed");
    expect(intelligence.profile?.beginAreaName).toBe("Antibes");
    expect(intelligence.releases[0]).toMatchObject({ title: "Fantasy", provenance: "broadCache" });
    expect(intelligence.sources).toHaveLength(3);
  });

  it("keeps unknown preview artists unmatched instead of guessing", async () => {
    const intelligence = await loadArtistIntelligence("College");
    expect(intelligence.matchState).toBe("unmatched");
    expect(intelligence.identity).toBeNull();
    expect(intelligence.releases).toEqual([]);
  });
});
