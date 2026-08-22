import { beforeEach, describe, expect, it } from "vitest";
import {
  loadArtistIntelligence,
  loadArtistReviewPage,
  resetMusicBrainzPreviewState,
  undoMusicBrainzCuration,
  updateArtistIdentityDecision,
  updateReleaseGroupDecision,
} from "./musicbrainz";

describe("MusicBrainz browser adapter", () => {
  beforeEach(resetMusicBrainzPreviewState);

  it("provides a truthful enriched preview for M83", async () => {
    const intelligence = await loadArtistIntelligence("M83");
    expect(intelligence.matchState).toBe("unconfirmed");
    expect(intelligence.profile?.beginAreaName).toBe("Antibes");
    expect(intelligence.releases[0]).toMatchObject({ title: "Fantasy", provenance: "broadCache" });
    expect(intelligence.sources).toHaveLength(3);
  });

  it("keeps unknown preview artists unmatched instead of guessing", async () => {
    const intelligence = await loadArtistIntelligence("Unknown Preview Artist");
    expect(intelligence.matchState).toBe("unmatched");
    expect(intelligence.identity).toBeNull();
    expect(intelligence.releases).toEqual([]);
  });

  it("persists confirm, release-link, undo, ignore, and clear decisions in preview state", async () => {
    const confirmed = await updateArtistIdentityDecision({
      action: "confirm",
      artist: "M83",
      mbid: "6d7b7cd4-254b-4c25-83f6-dd20f98ceacd",
    });
    expect(confirmed.identity?.provenance).toBe("auroraState");
    expect((await loadArtistIntelligence("M83")).decision?.decision).toBe("confirmed");

    const linked = await updateReleaseGroupDecision({
      action: "link",
      artist: "M83",
      artistMbid: confirmed.identity!.mbid,
      releaseMbid: confirmed.releases[0].mbid,
      localAlbumId: "preview-fantasy",
    });
    expect(linked.releases[0]).toMatchObject({ decision: "include", localAlbumId: "preview-fantasy" });
    expect((await undoMusicBrainzCuration())?.releases[0].decision).toBeNull();

    expect((await updateArtistIdentityDecision({ action: "ignore", artist: "M83" })).matchState).toBe("ignored");
    expect((await updateArtistIdentityDecision({ action: "clear", artist: "M83" })).matchState).toBe("unconfirmed");
  });

  it("pages and filters review candidates without leaking decided rows", async () => {
    const first = await loadArtistReviewPage({ pageSize: 1, filter: "needsReview" });
    expect(first.items).toHaveLength(1);
    expect(first.nextCursor).toBe("1");
    const second = await loadArtistReviewPage({ pageSize: 1, filter: "needsReview", cursor: first.nextCursor! });
    expect(second.items[0].artistKey).not.toBe(first.items[0].artistKey);

    await updateArtistIdentityDecision({ action: "ignore", artist: "M83" });
    const decided = await loadArtistReviewPage({ filter: "decided" });
    expect(decided.items.map((row) => row.artistKey)).toContain("m83");
    const needsReview = await loadArtistReviewPage({ filter: "needsReview" });
    expect(needsReview.items.map((row) => row.artistKey)).not.toContain("m83");
  });
});
