import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "./library";

export type MusicBrainzMatchState = "verified" | "unconfirmed" | "unmatched" | "conflict" | "ignored";
export type MusicBrainzSourceState = "connected" | "unavailable" | "browser-preview";

export interface MusicBrainzSource {
  id: "catalog" | "broadCache" | "curatedOverlay";
  label: string;
  status: MusicBrainzSourceState;
  detail: string;
}

export interface ArtistIdentity {
  mbid: string;
  canonicalName: string;
  matchMethod: string;
  confidence: number | null;
  provenance: "cacheExact" | "catalogImport" | "catalogOverlay" | "curatedOverlay";
  cacheNameCount: number | null;
}

export interface ArtistProfile {
  sortName: string | null;
  artistType: string | null;
  gender: string | null;
  lifeBeginDate: string | null;
  lifeEndDate: string | null;
  lifeEnded: boolean;
  areaName: string | null;
  beginAreaName: string | null;
  endAreaName: string | null;
  countryCode: string | null;
  countryName: string | null;
}

export interface MusicBrainzRelease {
  mbid: string;
  title: string;
  year: number | null;
  primaryType: string | null;
  secondaryTypes: string[];
  status: string | null;
  trackCount: number | null;
  provenance: "broadCache" | "catalogImport" | "curatedOverlay";
  decision: string | null;
  localAlbumId: string | null;
}

export interface ArtistIntelligence {
  artist: string;
  matchState: MusicBrainzMatchState;
  identity: ArtistIdentity | null;
  profile: ArtistProfile | null;
  releases: MusicBrainzRelease[];
  releasesTruncated: boolean;
  sources: MusicBrainzSource[];
}

const m83Preview: ArtistIntelligence = {
  artist: "M83",
  matchState: "unconfirmed",
  identity: {
    mbid: "6d7b7cd4-254b-4c25-83f6-dd20f98ceacd",
    canonicalName: "M83",
    matchMethod: "catalog-import",
    confidence: null,
    provenance: "catalogImport",
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
  releases: [
    ["Fantasy", 2023, "Album", []],
    ["Fantasy – Chapter 1", 2023, "EP", []],
    ["DSVII", 2019, "Album", []],
    ["Knife + Heart", 2019, "Album", ["Soundtrack"]],
    ["Junk", 2016, "Album", []],
    ["Hurry Up, We’re Dreaming", 2011, "Album", []],
    ["Saturdays = Youth", 2008, "Album", []],
  ].map(([title, year, primaryType, secondaryTypes], index) => ({
    mbid: `preview-release-${index}`,
    title: String(title),
    year: Number(year),
    primaryType: String(primaryType),
    secondaryTypes: secondaryTypes as string[],
    status: "Official",
    trackCount: null,
    provenance: "broadCache" as const,
    decision: null,
    localAlbumId: null,
  })),
  releasesTruncated: false,
  sources: [
    { id: "catalog", label: "Music Library catalog", status: "browser-preview", detail: "Browser preview data" },
    { id: "curatedOverlay", label: "Curated MusicBrainz overlay", status: "browser-preview", detail: "Browser preview data" },
    { id: "broadCache", label: "Broad MusicBrainz cache", status: "browser-preview", detail: "Browser preview data" },
  ],
};

function unmatchedPreview(artist: string): ArtistIntelligence {
  return {
    artist,
    matchState: "unmatched",
    identity: null,
    profile: null,
    releases: [],
    releasesTruncated: false,
    sources: m83Preview.sources,
  };
}

export async function loadArtistIntelligence(artist: string): Promise<ArtistIntelligence> {
  if (!isTauriRuntime()) {
    return artist.toLocaleLowerCase() === "m83" ? m83Preview : unmatchedPreview(artist);
  }
  return invoke<ArtistIntelligence>("artist_intelligence", { artist });
}
