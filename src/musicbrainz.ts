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
  provenance: "cacheExact" | "catalogImport" | "catalogOverlay" | "curatedOverlay" | "auroraState";
  cacheNameCount: number | null;
}

export interface ArtistCandidate {
  mbid: string;
  canonicalName: string;
  matchMethod: string;
  confidence: number | null;
  provenance: Exclude<ArtistIdentity["provenance"], "auroraState">;
  cacheNameCount: number | null;
  verifiedSource: boolean;
}

export interface ArtistCurationDecision {
  localArtistKey: string;
  displayArtist: string;
  decision: "confirmed" | "ignored";
  artistMbid: string | null;
  canonicalName: string | null;
  createdAtMs: number;
  updatedAtMs: number;
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
  decisionProvenance: "auroraState" | "curatedOverlay" | null;
  localAlbumId: string | null;
}

export interface ArtistIntelligence {
  artist: string;
  artistKey: string;
  matchState: MusicBrainzMatchState;
  identity: ArtistIdentity | null;
  candidates: ArtistCandidate[];
  decision: ArtistCurationDecision | null;
  hasExternalConflict: boolean;
  profile: ArtistProfile | null;
  releases: MusicBrainzRelease[];
  releasesTruncated: boolean;
  sources: MusicBrainzSource[];
}

export type ArtistReviewFilter = "needsReview" | "conflict" | "unconfirmed" | "decided" | "all";

export interface ArtistReviewItem {
  artistKey: string;
  displayArtist: string;
  matchState: MusicBrainzMatchState;
  identity: ArtistIdentity | null;
  candidates: ArtistCandidate[];
  decision: ArtistCurationDecision | null;
  hasExternalConflict: boolean;
}

export interface ArtistReviewPageRequest {
  pageSize?: number;
  cursor?: string;
  filter?: ArtistReviewFilter;
  search?: string;
}

export interface ArtistReviewPage {
  items: ArtistReviewItem[];
  nextCursor: string | null;
}

export type ArtistDecisionRequest =
  | { action: "confirm"; artist: string; mbid: string }
  | { action: "ignore"; artist: string }
  | { action: "clear"; artist: string };

export type ReleaseDecisionRequest =
  | { action: "link"; artist: string; artistMbid: string; releaseMbid: string; localAlbumId: string }
  | { action: "notInScope"; artist: string; artistMbid: string; releaseMbid: string }
  | { action: "ignore"; artist: string; artistMbid: string; releaseMbid: string }
  | { action: "clear"; artist: string; artistMbid: string; releaseMbid: string };

export interface CurationExportResult {
  path: string;
  artistDecisions: number;
  releaseDecisions: number;
  sourceRowsPreserved: boolean;
}

const m83Preview: ArtistIntelligence = {
  artist: "M83",
  artistKey: "m83",
  matchState: "unconfirmed",
  identity: {
    mbid: "6d7b7cd4-254b-4c25-83f6-dd20f98ceacd",
    canonicalName: "M83",
    matchMethod: "catalog-import",
    confidence: null,
    provenance: "catalogImport",
    cacheNameCount: 1,
  },
  candidates: [{
    mbid: "6d7b7cd4-254b-4c25-83f6-dd20f98ceacd",
    canonicalName: "M83",
    matchMethod: "catalog-import",
    confidence: null,
    provenance: "catalogImport",
    cacheNameCount: 1,
    verifiedSource: false,
  }],
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
  releases: [
    ["Fantasy", 2023, "Album", []],
    ["Fantasy – Chapter 1", 2023, "EP", []],
    ["DSVII", 2019, "Album", []],
    ["Knife + Heart", 2019, "Album", ["Soundtrack"]],
    ["Junk", 2016, "Album", []],
    ["Hurry Up, We’re Dreaming", 2011, "Album", []],
    ["Saturdays = Youth", 2008, "Album", []],
  ].map(([title, year, primaryType, secondaryTypes], index) => ({
    mbid: `00000000-0000-0000-0000-${String(index + 1).padStart(12, "0")}`,
    title: String(title),
    year: Number(year),
    primaryType: String(primaryType),
    secondaryTypes: secondaryTypes as string[],
    status: "Official",
    trackCount: null,
    provenance: "broadCache" as const,
    decision: null,
    decisionProvenance: null,
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
    artistKey: artist.trim().toLocaleLowerCase(),
    matchState: "unmatched",
    identity: null,
    candidates: [],
    decision: null,
    hasExternalConflict: false,
    profile: null,
    releases: [],
    releasesTruncated: false,
    sources: m83Preview.sources,
  };
}

const daftPunkPreview: ArtistIntelligence = {
  ...unmatchedPreview("Daft Punk"),
  artistKey: "daft punk",
  matchState: "conflict",
  candidates: [
    {
      mbid: "056e4f3e-d505-4dad-8ec1-d04f521cbb56",
      canonicalName: "Daft Punk",
      matchMethod: "catalog-import",
      confidence: null,
      provenance: "catalogImport",
      cacheNameCount: 1,
      verifiedSource: false,
    },
    {
      mbid: "22222222-2222-2222-2222-222222222222",
      canonicalName: "Daft Punk",
      matchMethod: "exact-name-cache",
      confidence: null,
      provenance: "cacheExact",
      cacheNameCount: 2,
      verifiedSource: false,
    },
  ],
  hasExternalConflict: true,
};

const collegePreview: ArtistIntelligence = {
  ...unmatchedPreview("College"),
  artistKey: "college",
  matchState: "unconfirmed",
  identity: {
    mbid: "33333333-3333-3333-3333-333333333333",
    canonicalName: "College",
    matchMethod: "exact-name-cache",
    confidence: null,
    provenance: "cacheExact",
    cacheNameCount: 1,
  },
  candidates: [{
    mbid: "33333333-3333-3333-3333-333333333333",
    canonicalName: "College",
    matchMethod: "exact-name-cache",
    confidence: null,
    provenance: "cacheExact",
    cacheNameCount: 1,
    verifiedSource: false,
  }],
};

type PreviewReleaseDecision = {
  decision: "include" | "not-in-scope" | "ignored";
  localAlbumId: string | null;
};

type PreviewHistory =
  | { kind: "artist"; key: string; artist: string; before: ArtistCurationDecision | null }
  | { kind: "release"; key: string; artist: string; before: PreviewReleaseDecision | null };

const previewArtistDecisions = new Map<string, ArtistCurationDecision>();
const previewReleaseDecisions = new Map<string, PreviewReleaseDecision>();
const previewHistory: PreviewHistory[] = [];

function previewBase(artist: string): ArtistIntelligence {
  const key = artist.trim().toLocaleLowerCase();
  const base = key === "m83"
    ? m83Preview
    : key === "daft punk"
      ? daftPunkPreview
      : key === "college"
        ? collegePreview
        : unmatchedPreview(artist);
  return structuredClone(base);
}

function previewIntelligence(artist: string): ArtistIntelligence {
  const intelligence = previewBase(artist);
  const decision = previewArtistDecisions.get(intelligence.artistKey) ?? null;
  intelligence.decision = decision ? { ...decision } : null;
  if (decision?.decision === "ignored") {
    intelligence.matchState = "ignored";
    intelligence.identity = null;
  } else if (decision?.decision === "confirmed" && decision.artistMbid) {
    const candidate = intelligence.candidates.find((row) => row.mbid === decision.artistMbid);
    intelligence.matchState = "verified";
    intelligence.identity = {
      mbid: decision.artistMbid,
      canonicalName: decision.canonicalName ?? candidate?.canonicalName ?? intelligence.artist,
      matchMethod: "aurora-confirmed",
      confidence: 1,
      provenance: "auroraState",
      cacheNameCount: candidate?.cacheNameCount ?? null,
    };
  }
  intelligence.releases = intelligence.releases.map((release) => {
    const saved = previewReleaseDecisions.get(`${intelligence.artistKey}:${release.mbid}`);
    return saved ? {
      ...release,
      decision: saved.decision,
      decisionProvenance: "auroraState",
      localAlbumId: saved.localAlbumId,
    } : release;
  });
  return intelligence;
}

export async function loadArtistIntelligence(artist: string): Promise<ArtistIntelligence> {
  if (!isTauriRuntime()) {
    return previewIntelligence(artist);
  }
  return invoke<ArtistIntelligence>("artist_intelligence", { artist });
}

export async function loadArtistReviewPage(request: ArtistReviewPageRequest): Promise<ArtistReviewPage> {
  if (!isTauriRuntime()) {
    const filter = request.filter ?? "needsReview";
    const search = request.search?.trim().toLocaleLowerCase() ?? "";
    const rows = ["Daft Punk", "M83", "College"]
      .map(previewIntelligence)
      .filter((row) => !search || row.artist.toLocaleLowerCase().includes(search))
      .filter((row) => {
        if (filter === "all") return true;
        if (filter === "decided") return row.decision !== null;
        if (filter === "conflict") return row.decision === null && row.matchState === "conflict";
        if (filter === "unconfirmed") return row.decision === null && row.matchState === "unconfirmed";
        return row.decision === null && ["conflict", "unconfirmed", "unmatched"].includes(row.matchState);
      });
    const start = Math.max(0, Number(request.cursor ?? 0) || 0);
    const size = Math.max(1, Math.min(100, request.pageSize ?? 50));
    const pageRows = rows.slice(start, start + size);
    return {
      items: pageRows.map((row) => ({
        artistKey: row.artistKey,
        displayArtist: row.artist,
        matchState: row.matchState,
        identity: row.identity,
        candidates: row.candidates,
        decision: row.decision,
        hasExternalConflict: row.hasExternalConflict,
      })),
      nextCursor: start + size < rows.length ? String(start + size) : null,
    };
  }
  return invoke<ArtistReviewPage>("musicbrainz_review_page", { request });
}

export async function updateArtistIdentityDecision(request: ArtistDecisionRequest): Promise<ArtistIntelligence> {
  if (!isTauriRuntime()) {
    const current = previewIntelligence(request.artist);
    const key = current.artistKey;
    previewHistory.push({ kind: "artist", key, artist: current.artist, before: previewArtistDecisions.get(key) ?? null });
    if (request.action === "clear") {
      previewArtistDecisions.delete(key);
    } else if (request.action === "ignore") {
      const now = Date.now();
      previewArtistDecisions.set(key, {
        localArtistKey: key,
        displayArtist: current.artist,
        decision: "ignored",
        artistMbid: null,
        canonicalName: null,
        createdAtMs: now,
        updatedAtMs: now,
      });
    } else {
      const candidate = current.candidates.find((row) => row.mbid === request.mbid);
      if (!candidate) throw new Error("That MusicBrainz candidate is no longer available.");
      const now = Date.now();
      previewArtistDecisions.set(key, {
        localArtistKey: key,
        displayArtist: current.artist,
        decision: "confirmed",
        artistMbid: candidate.mbid,
        canonicalName: candidate.canonicalName,
        createdAtMs: now,
        updatedAtMs: now,
      });
    }
    return previewIntelligence(request.artist);
  }
  return invoke<ArtistIntelligence>("update_artist_identity_decision", { request });
}

export async function updateReleaseGroupDecision(request: ReleaseDecisionRequest): Promise<ArtistIntelligence> {
  if (!isTauriRuntime()) {
    const current = previewIntelligence(request.artist);
    if (!current.identity || current.identity.mbid !== request.artistMbid || !["auroraState", "curatedOverlay", "catalogOverlay"].includes(current.identity.provenance)) {
      throw new Error("Confirm the artist identity before curating release groups.");
    }
    const release = current.releases.find((row) => row.mbid === request.releaseMbid);
    if (!release) throw new Error("That release group is no longer available.");
    const key = `${current.artistKey}:${request.releaseMbid}`;
    previewHistory.push({ kind: "release", key, artist: current.artist, before: previewReleaseDecisions.get(key) ?? null });
    if (request.action === "clear") {
      previewReleaseDecisions.delete(key);
    } else {
      previewReleaseDecisions.set(key, {
        decision: request.action === "link" ? "include" : request.action === "notInScope" ? "not-in-scope" : "ignored",
        localAlbumId: request.action === "link" ? request.localAlbumId : null,
      });
    }
    return previewIntelligence(request.artist);
  }
  return invoke<ArtistIntelligence>("update_release_group_decision", { request });
}

export async function undoMusicBrainzCuration(): Promise<ArtistIntelligence | null> {
  if (!isTauriRuntime()) {
    const entry = previewHistory.pop();
    if (!entry) return null;
    if (entry.kind === "artist") {
      if (entry.before) previewArtistDecisions.set(entry.key, entry.before);
      else previewArtistDecisions.delete(entry.key);
    } else if (entry.before) {
      previewReleaseDecisions.set(entry.key, entry.before);
    } else {
      previewReleaseDecisions.delete(entry.key);
    }
    return previewIntelligence(entry.artist);
  }
  return invoke<ArtistIntelligence | null>("undo_musicbrainz_curation");
}

export async function exportMusicBrainzCuration(): Promise<CurationExportResult> {
  if (!isTauriRuntime()) {
    return {
      path: "C:\\Users\\Jorn\\AppData\\Roaming\\Aurora\\exports\\aurora-musicbrainz-overlay-preview.sqlite3",
      artistDecisions: previewArtistDecisions.size,
      releaseDecisions: previewReleaseDecisions.size,
      sourceRowsPreserved: true,
    };
  }
  return invoke<CurationExportResult>("export_musicbrainz_curation");
}

export function resetMusicBrainzPreviewState(): void {
  previewArtistDecisions.clear();
  previewReleaseDecisions.clear();
  previewHistory.length = 0;
}
