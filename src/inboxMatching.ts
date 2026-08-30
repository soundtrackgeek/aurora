import type { InboxTrack, ReleaseTrack } from "./inbox";

export type InboxTrackMatchStatus = "exact" | "likely" | "confirmed" | "ambiguous" | "extra" | "missing";

export interface InboxManualTrackMatch {
  localIndex: number;
  releaseIndex: number;
}

export interface InboxTrackMatch {
  localIndex: number | null;
  releaseIndex: number | null;
  status: InboxTrackMatchStatus;
}

export interface InboxTrackReconciliation {
  rows: InboxTrackMatch[];
  matchedCount: number;
  exactCount: number;
  likelyCount: number;
  confirmedCount: number;
  extraCount: number;
  missingCount: number;
  ambiguousCount: number;
  cleanupSafe: boolean;
}

const LEADING_ARTICLE = /^(?:a|an|the)\s+/u;
const DOTTED_INITIALISM = /(?<![\p{L}\p{N}])(?:[\p{L}\p{N}]\s*\.\s*)+[\p{L}\p{N}](?![\p{L}\p{N}])/gu;
const SINGLE_CHARACTER_TOKEN = /^[\p{L}\p{N}]$/u;

function collapseInitialismRuns(value: string): string {
  const tokens = value.split(" ").filter(Boolean);
  const collapsed: string[] = [];
  for (let index = 0; index < tokens.length;) {
    let end = index;
    while (end < tokens.length && SINGLE_CHARACTER_TOKEN.test(tokens[end])) end += 1;
    if (end - index >= 3) {
      collapsed.push(tokens.slice(index, end).join(""));
      index = end;
    } else {
      collapsed.push(tokens[index]);
      index += 1;
    }
  }
  return collapsed.join(" ");
}

export function normalizeInboxTrackTitle(value: string | null | undefined): string {
  const normalized = (value ?? "")
    .toLocaleLowerCase()
    .replace(/\([^)]*(?:bonus|remaster(?:ed)?|version)[^)]*\)/gu, " ")
    .replace(DOTTED_INITIALISM, (initialism) => initialism.replace(/[.\s]+/gu, ""))
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim();
  return collapseInitialismRuns(normalized).replace(LEADING_ARTICLE, "");
}

function titleTokens(value: string): Set<string> {
  return new Set(value.split(" ").filter(Boolean));
}

function titleSimilarity(left: string, right: string): number {
  if (!left || !right) return 0;
  if (left === right) return 1;
  const leftTokens = titleTokens(left);
  const rightTokens = titleTokens(right);
  let intersection = 0;
  for (const token of leftTokens) {
    if (rightTokens.has(token)) intersection += 1;
  }
  const union = new Set([...leftTokens, ...rightTokens]).size;
  return union ? intersection / union : 0;
}

function durationDifference(local: InboxTrack, release: ReleaseTrack): number | null {
  if (local.durationMs === null || local.durationMs === undefined || release.durationMs === null) return null;
  return Math.abs(local.durationMs - release.durationMs);
}

function exactCandidate(
  localTracks: readonly InboxTrack[],
  releaseTrack: ReleaseTrack,
  availableLocalIndices: readonly number[],
): { localIndex: number | null; ambiguous: boolean } {
  const normalizedReleaseTitle = normalizeInboxTrackTitle(releaseTrack.title);
  const titleMatches = availableLocalIndices.filter((localIndex) => (
    normalizeInboxTrackTitle(localTracks[localIndex]?.title ?? localTracks[localIndex]?.fileName) === normalizedReleaseTitle
  ));
  if (titleMatches.length === 1) return { localIndex: titleMatches[0], ambiguous: false };
  if (titleMatches.length < 2) return { localIndex: null, ambiguous: false };

  const numberedMatches = titleMatches.filter((localIndex) => {
    const local = localTracks[localIndex];
    return local?.trackNumber === releaseTrack.trackNumber
      && (releaseTrack.discNumber === null || local.discNumber === releaseTrack.discNumber);
  });
  if (numberedMatches.length === 1) return { localIndex: numberedMatches[0], ambiguous: false };

  const durationMatches = titleMatches
    .map((localIndex) => ({ localIndex, difference: durationDifference(localTracks[localIndex], releaseTrack) }))
    .filter((candidate): candidate is { localIndex: number; difference: number } => candidate.difference !== null)
    .sort((left, right) => left.difference - right.difference);
  if (durationMatches[0] && durationMatches[0].difference <= 8_000
    && (!durationMatches[1] || durationMatches[1].difference - durationMatches[0].difference >= 2_000)) {
    return { localIndex: durationMatches[0].localIndex, ambiguous: false };
  }
  return { localIndex: null, ambiguous: true };
}

export function reconcileInboxTracks(
  localTracks: readonly InboxTrack[],
  releaseTracks: readonly ReleaseTrack[],
  manualMatches: readonly InboxManualTrackMatch[] = [],
): InboxTrackReconciliation {
  const matchedLocal = new Set<number>();
  const matchedRelease = new Set<number>();
  const matches: InboxTrackMatch[] = [];
  const ambiguousReleaseIndices = new Set<number>();

  for (const manual of manualMatches) {
    if (manual.localIndex < 0 || manual.localIndex >= localTracks.length
      || manual.releaseIndex < 0 || manual.releaseIndex >= releaseTracks.length
      || matchedLocal.has(manual.localIndex) || matchedRelease.has(manual.releaseIndex)) continue;
    matchedLocal.add(manual.localIndex);
    matchedRelease.add(manual.releaseIndex);
    matches.push({ localIndex: manual.localIndex, releaseIndex: manual.releaseIndex, status: "confirmed" });
  }

  for (let releaseIndex = 0; releaseIndex < releaseTracks.length; releaseIndex += 1) {
    const available = localTracks.map((_, index) => index).filter((index) => !matchedLocal.has(index));
    const candidate = exactCandidate(localTracks, releaseTracks[releaseIndex], available);
    if (candidate.ambiguous) {
      ambiguousReleaseIndices.add(releaseIndex);
    } else if (candidate.localIndex !== null) {
      matchedLocal.add(candidate.localIndex);
      matchedRelease.add(releaseIndex);
      matches.push({ localIndex: candidate.localIndex, releaseIndex, status: "exact" });
    }
  }

  for (let releaseIndex = 0; releaseIndex < releaseTracks.length; releaseIndex += 1) {
    if (matchedRelease.has(releaseIndex) || ambiguousReleaseIndices.has(releaseIndex)) continue;
    const localIndex = releaseIndex;
    if (localIndex >= localTracks.length || matchedLocal.has(localIndex)) continue;
    const local = localTracks[localIndex];
    const release = releaseTracks[releaseIndex];
    const similarity = titleSimilarity(
      normalizeInboxTrackTitle(local.title ?? local.fileName),
      normalizeInboxTrackTitle(release.title),
    );
    const duration = durationDifference(local, release);
    if (similarity >= 0.6 || (similarity >= 0.45 && duration !== null && duration <= 8_000)) {
      matchedLocal.add(localIndex);
      matchedRelease.add(releaseIndex);
      matches.push({ localIndex, releaseIndex, status: "likely" });
    }
  }

  const rows = [
    ...matches.sort((left, right) => (left.localIndex ?? 0) - (right.localIndex ?? 0)),
    ...[...ambiguousReleaseIndices].map((releaseIndex): InboxTrackMatch => ({ localIndex: null, releaseIndex, status: "ambiguous" })),
    ...releaseTracks.map((_, releaseIndex) => releaseIndex)
      .filter((releaseIndex) => !matchedRelease.has(releaseIndex) && !ambiguousReleaseIndices.has(releaseIndex))
      .map((releaseIndex): InboxTrackMatch => ({ localIndex: null, releaseIndex, status: "missing" })),
    ...localTracks.map((_, localIndex) => localIndex)
      .filter((localIndex) => !matchedLocal.has(localIndex))
      .map((localIndex): InboxTrackMatch => ({ localIndex, releaseIndex: null, status: "extra" })),
  ];
  const exactCount = matches.filter((match) => match.status === "exact").length;
  const likelyCount = matches.filter((match) => match.status === "likely").length;
  const confirmedCount = matches.filter((match) => match.status === "confirmed").length;
  const extraCount = localTracks.length - matchedLocal.size;
  const missingCount = releaseTracks.length - matchedRelease.size - ambiguousReleaseIndices.size;
  const ambiguousCount = ambiguousReleaseIndices.size;
  return {
    rows,
    matchedCount: matches.length,
    exactCount,
    likelyCount,
    confirmedCount,
    extraCount,
    missingCount,
    ambiguousCount,
    cleanupSafe: releaseTracks.length > 0
      && matches.length === releaseTracks.length
      && missingCount === 0
      && ambiguousCount === 0,
  };
}
