import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ArtistDetail } from "../../library";
import type { ArtistIntelligence } from "../../musicbrainz";
import { ArtistWorld } from "./ArtistWorld";

const detail: ArtistDetail = {
  artist: { id: "m83", name: "M83", trackCount: 94, albumCount: 9, playCount: 4218, lastPlayedAtMs: 1_777_680_540_000 },
  albums: [{ id: "mb:fantasy", title: "Fantasy", artist: "M83", releaseYear: 2023, genre: "Electronic", totalTracks: 13, ratedTracks: 4, lovedTracks: 2, durationSeconds: 3200, rating: 4, albumScore: null }],
  albumsTruncated: false,
};

const intelligence: ArtistIntelligence = {
  artist: "M83",
  artistKey: "m83",
  matchState: "verified",
  identity: {
    mbid: "6d7b7cd4-254b-4c25-83f6-dd20f98ceacd",
    canonicalName: "M83",
    matchMethod: "manual-mbid",
    confidence: 1,
    provenance: "curatedOverlay",
    cacheNameCount: 1,
  },
  candidates: [],
  decision: null,
  hasExternalConflict: false,
  profile: {
    sortName: "M83",
    artistType: "Group",
    gender: null,
    lifeBeginDate: "2001",
    lifeEndDate: null,
    lifeEnded: false,
    areaName: "France",
    beginAreaName: "Antibes",
    endAreaName: null,
    countryCode: "FR",
    countryName: "France",
  },
  releases: [{
    mbid: "release-1",
    title: "Fantasy",
    year: 2023,
    primaryType: "Album",
    secondaryTypes: [],
    status: "Official",
    trackCount: null,
    provenance: "curatedOverlay",
    decision: "included",
    decisionProvenance: "curatedOverlay",
    localAlbumId: "album-1",
  }],
  releasesTruncated: false,
  sources: [
    { id: "curatedOverlay", label: "Curated overlay", status: "connected", detail: "Read-only" },
    { id: "broadCache", label: "Broad cache", status: "connected", detail: "Read-only" },
  ],
};

const handlers = {
  onRetry: vi.fn(),
  onExploreLibrary: vi.fn(),
  onArtistDecision: vi.fn(),
  onReleaseDecision: vi.fn(),
};

afterEach(cleanup);

describe("ArtistWorld", () => {
  it("renders provenance, catalog counts, releases, and decisions", () => {
    render(<ArtistWorld artistName="M83" catalogDetail={detail} intelligence={intelligence} state="ready" {...handlers} />);
    expect(screen.getByRole("heading", { name: "M83" })).toBeInTheDocument();
    expect(screen.getByText("Curated identity")).toBeInTheDocument();
    expect(screen.getByLabelText("MusicBrainz artist profile")).toHaveTextContent("Antibes");
    expect(screen.getByText("Fantasy")).toBeInTheDocument();
    expect(screen.getByText("included")).toBeInTheDocument();
    expect(screen.getByLabelText("Local catalog summary")).toHaveTextContent("94");
  });

  it("keeps exploration available while enrichment is loading", () => {
    const onExploreLibrary = vi.fn();
    render(<ArtistWorld artistName="M83" catalogDetail={detail} intelligence={null} state="loading" {...handlers} onExploreLibrary={onExploreLibrary} />);
    expect(screen.getByRole("status")).toHaveTextContent("No online request");
    fireEvent.click(screen.getByRole("button", { name: "Explore this artist in Aurora" }));
    expect(onExploreLibrary).toHaveBeenCalledOnce();
  });

  it("surfaces a scoped error and retries without replacing catalog context", () => {
    const onRetry = vi.fn();
    render(<ArtistWorld artistName="M83" catalogDetail={detail} intelligence={null} state="error" errorMessage="Cache unavailable" {...handlers} onRetry={onRetry} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Cache unavailable");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(onRetry).toHaveBeenCalledOnce();
    expect(screen.getByLabelText("Local catalog summary")).toBeInTheDocument();
  });

  it("shows degraded context and never turns an unknown start date into present", () => {
    render(
      <ArtistWorld
        artistName="M83"
        catalogDetail={detail}
        intelligence={{ ...intelligence, profile: { ...intelligence.profile!, lifeBeginDate: null } }}
        state="ready"
        errorMessage="The catalog summary is unavailable."
        {...handlers}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("Partial local context");
    expect(screen.getByLabelText("MusicBrainz artist profile")).toHaveTextContent("Unknown");
    expect(screen.getByLabelText("MusicBrainz artist profile")).not.toHaveTextContent("Unknown–present");
    expect(screen.getByText(/Curated overlay · manual-mbid/)).toBeInTheDocument();
    expect(screen.getByText(/Curated overlay · Official/)).toBeInTheDocument();
  });

  it("confirms a selected candidate and edits a release only through explicit callbacks", () => {
    const onArtistDecision = vi.fn();
    const onReleaseDecision = vi.fn();
    const candidate = {
      mbid: intelligence.identity!.mbid,
      canonicalName: "M83",
      matchMethod: "catalog-import",
      confidence: null,
      provenance: "catalogImport" as const,
      cacheNameCount: 1,
      verifiedSource: false,
    };
    const { rerender } = render(
      <ArtistWorld
        artistName="M83"
        catalogDetail={detail}
        intelligence={{ ...intelligence, matchState: "unconfirmed", identity: { ...intelligence.identity!, provenance: "catalogImport" }, candidates: [candidate] }}
        state="ready"
        {...handlers}
        onArtistDecision={onArtistDecision}
        onReleaseDecision={onReleaseDecision}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Confirm candidate" }));
    expect(onArtistDecision).toHaveBeenCalledWith({ action: "confirm", artist: "M83", mbid: candidate.mbid });

    rerender(
      <ArtistWorld
        artistName="M83"
        catalogDetail={detail}
        intelligence={{ ...intelligence, identity: { ...intelligence.identity!, provenance: "auroraState" }, decision: { localArtistKey: "m83", displayArtist: "M83", decision: "confirmed", artistMbid: candidate.mbid, canonicalName: "M83", createdAtMs: 1, updatedAtMs: 1 } }}
        state="ready"
        {...handlers}
        onArtistDecision={onArtistDecision}
        onReleaseDecision={onReleaseDecision}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "included" }));
    fireEvent.change(screen.getByLabelText("Local album"), { target: { value: "mb:fantasy" } });
    fireEvent.click(screen.getByRole("button", { name: "Link" }));
    expect(onReleaseDecision).toHaveBeenCalledWith({
      action: "link",
      artist: "M83",
      artistMbid: candidate.mbid,
      releaseMbid: "release-1",
      localAlbumId: "mb:fantasy",
    });
  });
});
