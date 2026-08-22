import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ArtistDetail } from "../../library";
import type { ArtistIntelligence } from "../../musicbrainz";
import { ArtistWorld } from "./ArtistWorld";

const detail: ArtistDetail = {
  artist: { id: "m83", name: "M83", trackCount: 94, albumCount: 9, playCount: 4218 },
  albums: [],
  albumsTruncated: false,
};

const intelligence: ArtistIntelligence = {
  artist: "M83",
  matchState: "verified",
  identity: {
    mbid: "6d7b7cd4-254b-4c25-83f6-dd20f98ceacd",
    canonicalName: "M83",
    matchMethod: "manual-mbid",
    confidence: 1,
    provenance: "curatedOverlay",
    cacheNameCount: 1,
  },
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
    localAlbumId: "album-1",
  }],
  releasesTruncated: false,
  sources: [
    { id: "curatedOverlay", label: "Curated overlay", status: "connected", detail: "Read-only" },
    { id: "broadCache", label: "Broad cache", status: "connected", detail: "Read-only" },
  ],
};

afterEach(cleanup);

describe("ArtistWorld", () => {
  it("renders provenance, catalog counts, releases, and decisions", () => {
    render(<ArtistWorld artistName="M83" catalogDetail={detail} intelligence={intelligence} state="ready" onRetry={vi.fn()} onExploreLibrary={vi.fn()} />);
    expect(screen.getByRole("heading", { name: "M83" })).toBeInTheDocument();
    expect(screen.getByText("Curated identity")).toBeInTheDocument();
    expect(screen.getByLabelText("MusicBrainz artist profile")).toHaveTextContent("Antibes");
    expect(screen.getByText("Fantasy")).toBeInTheDocument();
    expect(screen.getByText("included")).toBeInTheDocument();
    expect(screen.getByLabelText("Local catalog summary")).toHaveTextContent("94");
  });

  it("keeps exploration available while enrichment is loading", () => {
    const onExploreLibrary = vi.fn();
    render(<ArtistWorld artistName="M83" catalogDetail={detail} intelligence={null} state="loading" onRetry={vi.fn()} onExploreLibrary={onExploreLibrary} />);
    expect(screen.getByRole("status")).toHaveTextContent("No online request");
    fireEvent.click(screen.getByRole("button", { name: "Explore this artist in Aurora" }));
    expect(onExploreLibrary).toHaveBeenCalledOnce();
  });

  it("surfaces a scoped error and retries without replacing catalog context", () => {
    const onRetry = vi.fn();
    render(<ArtistWorld artistName="M83" catalogDetail={detail} intelligence={null} state="error" errorMessage="Cache unavailable" onRetry={onRetry} onExploreLibrary={vi.fn()} />);
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
        onRetry={vi.fn()}
        onExploreLibrary={vi.fn()}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("Partial local context");
    expect(screen.getByLabelText("MusicBrainz artist profile")).toHaveTextContent("Unknown");
    expect(screen.getByLabelText("MusicBrainz artist profile")).not.toHaveTextContent("Unknown–present");
    expect(screen.getByText(/Curated overlay · manual-mbid/)).toBeInTheDocument();
    expect(screen.getByText(/Curated overlay · Official/)).toBeInTheDocument();
  });
});
