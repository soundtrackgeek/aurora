import { useEffect, useLayoutEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";
import type { SidebarDestination } from "../../components/navigation/SidebarNavigation";
import type { GenreSummary } from "../../genres";
import {
  applyAlbumPopularity, applyAlbumTrackMetricsProjection, applyAlbumTrackTagProjection,
  applyEditableTrackTagProjection, applyTrackTagProjection, loadAlbumDetail,
  type LibrarySnapshot, type Track,
} from "../../library";
import type { usePlayback } from "../../playback";
import { listenForGlobalShortcutResults, type GlobalShortcutResult } from "../../shortcuts";
import { tagValuesForTrack, trackWithTagValues, updateTrackTags, type CatalogSync, type TagValues } from "../../tags";
import type { useCatalogProjection } from "../catalog/useCatalogProjection";
import type { usePendingLibrarySync } from "../catalog/usePendingLibrarySync";
import type { useGenresDomain } from "../domains/useGenresDomain";
import type { usePublishersDomain } from "../domains/usePublishersDomain";
import type { useRatingsDomain } from "../domains/useRatingsDomain";
import type { useYearsDomain } from "../domains/useYearsDomain";
import type { ExplorerWorkspace } from "../explorer/useExplorerWorkspace";
import type { InspectorSelection } from "../inspector/useInspectorSelection";
import type { WorkspaceRestoration } from "../navigation/useWorkspaceRestoration";

export interface TagMutationOptions {
  snapshot: LibrarySnapshot | null;
  setSnapshot: Dispatch<SetStateAction<LibrarySnapshot | null>>;
  selectedTrack: Track | null;
  setSelectedTrack: Dispatch<SetStateAction<Track | null>>;
  activeNav: SidebarDestination;
  setSyncMessage: Dispatch<SetStateAction<string | null>>;
  explorer: Pick<ExplorerWorkspace, "explorerTracks" | "explorerFilters" | "setExplorerTracks"
    | "setExplorerAlbums" | "setExplorerReloadToken" | "preserveExplorerOnReloadRef" | "pendingExplorerAlbumIdRef">;
  inspector: Pick<InspectorSelection, "selectedAlbumId" | "selectedAlbumIdRef" | "albumTracks" | "setAlbumTracks"
    | "setAlbumTracksTruncated" | "setAlbumDetailState" | "albumRequestRef">;
  workspace: Pick<WorkspaceRestoration, "mainScrollRef" | "scrollPositionByDestinationRef" | "restoringScrollRef">;
  genres: Pick<ReturnType<typeof useGenresDomain>, "setGenreDetail" | "setGenreAtlasGenres"
    | "setGenreIndexReloadToken" | "setGenreDetailReloadToken">;
  years: Pick<ReturnType<typeof useYearsDomain>, "yearAlbumTracks" | "setYearAlbumTracks">;
  ratings: Pick<ReturnType<typeof useRatingsDomain>, "ratingAlbumTracks" | "setRatingAlbumTracks">;
  publishers: Pick<ReturnType<typeof usePublishersDomain>, "publisherAlbumTracks" | "setPublisherAlbumTracks">;
  projection: Pick<ReturnType<typeof useCatalogProjection>, "acceptTrackProjectionKeys" | "latestTrackProjectionTokensRef">;
  handleCatalogSync: ReturnType<typeof usePendingLibrarySync>["handleCatalogSync"];
  playback: Pick<ReturnType<typeof usePlayback>, "refreshTrack" | "applySnapshot">;
}

function genreSummaryWithTrackChange(
  summary: GenreSummary,
  before: Track,
  after: Track,
): GenreSummary {
  if (summary.name !== before.genre || before.genre !== after.genre) return summary;
  const ratedTracks = Math.max(
    0,
    summary.ratedTracks + Number(after.rating !== null) - Number(before.rating !== null),
  );
  const ratingSum = (summary.averageRating ?? 0) * summary.ratedTracks
    - (before.rating ?? 0)
    + (after.rating ?? 0);
  return {
    ...summary,
    ratedTracks,
    averageRating: ratedTracks > 0 ? Math.min(5, Math.max(0, ratingSum / ratedTracks)) : null,
    lovedTracks: Math.max(0, summary.lovedTracks + Number(after.loved) - Number(before.loved)),
  };
}

/** Applies verified and optimistic MP3 edits consistently across visible and retained caches. */
export function useTagMutations({
  snapshot, setSnapshot, selectedTrack, setSelectedTrack, activeNav, setSyncMessage,
  explorer, inspector, workspace, genres, years, ratings, publishers, projection,
  handleCatalogSync, playback,
}: TagMutationOptions) {
  const { explorerTracks, explorerFilters, setExplorerTracks, setExplorerAlbums,
    setExplorerReloadToken, preserveExplorerOnReloadRef } = explorer;
  const { selectedAlbumId, selectedAlbumIdRef, albumTracks, setAlbumTracks, setAlbumTracksTruncated,
    setAlbumDetailState, albumRequestRef } = inspector;
  const { mainScrollRef, scrollPositionByDestinationRef, restoringScrollRef } = workspace;
  const { setGenreDetail, setGenreAtlasGenres, setGenreIndexReloadToken, setGenreDetailReloadToken } = genres;
  const { yearAlbumTracks, setYearAlbumTracks } = years;
  const { ratingAlbumTracks, setRatingAlbumTracks } = ratings;
  const { publisherAlbumTracks, setPublisherAlbumTracks } = publishers;
  const { acceptTrackProjectionKeys, latestTrackProjectionTokensRef } = projection;
  const [inlineSavingKeys, setInlineSavingKeys] = useState<Set<string>>(() => new Set());
  const [inlineTagRevisions, setInlineTagRevisions] = useState<Record<string, number>>({});
  const inlineSaveRef = useRef<Set<string>>(new Set());
  const shortcutResultHandlerRef = useRef<(result: GlobalShortcutResult) => void>(() => undefined);

  useEffect(() => {
    let cancelled = false;
    let unlisten: () => void = () => undefined;
    void listenForGlobalShortcutResults((result) => shortcutResultHandlerRef.current(result))
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch((error: unknown) => console.warn("Aurora could not listen for shortcut results", error));
    return () => {
      cancelled = true;
      unlisten();
    };
  }, []);

  function applyTrackChanges(updatedTracks: Track[], sync?: CatalogSync): boolean {
    const projection = acceptTrackProjectionKeys(
      updatedTracks.map((track) => track.trackKey),
      sync?.projectionToken,
    );
    const acceptedTracks = updatedTracks.filter((track) => (
      projection.acceptedTrackKeys.has(track.trackKey)
    ));
    if (acceptedTracks.length === 0) return projection.complete;
    const updatedByKey = new Map(acceptedTracks.map((track) => [track.trackKey, track]));
    const project = (track: Track) => {
      const updated = updatedByKey.get(track.trackKey);
      return updated ? applyEditableTrackTagProjection(track, updated) : track;
    };
    const knownTracks = [
      ...(selectedTrack ? [selectedTrack] : []),
      ...explorerTracks,
      ...albumTracks,
      ...yearAlbumTracks,
      ...ratingAlbumTracks,
      ...publisherAlbumTracks,
      ...(snapshot?.tracks ?? []),
    ];
    const baselines = new Map(knownTracks.map((track) => [track.trackKey, track]));
    if (explorerFilters.query.trim() && acceptedTracks.some((track) => {
      const baseline = baselines.get(track.trackKey);
      return baseline && baseline.genre !== track.genre;
    })) {
      const currentScroll = mainScrollRef.current?.scrollTop;
      if (typeof currentScroll === "number" && currentScroll > 0) {
        scrollPositionByDestinationRef.current[activeNav] = currentScroll;
      }
      restoringScrollRef.current = true;
      preserveExplorerOnReloadRef.current = true;
      setExplorerReloadToken((value) => value + 1);
    }

    setSelectedTrack((current) => current ? project(current) : current);
    setExplorerTracks((current) => current.map(project));
    setExplorerAlbums((current) => current.map((album) => (
      applyAlbumTrackTagProjection(album, acceptedTracks)
    )));
    setAlbumTracks((current) => current.map(project));
    setYearAlbumTracks((current) => current.map(project));
    setRatingAlbumTracks((current) => current.map(project));
    setPublisherAlbumTracks((current) => current.map(project));
    setGenreDetail((current) => {
      if (!current) return current;
      const summary = acceptedTracks.reduce((next, updated) => {
        const baseline = baselines.get(updated.trackKey);
        return baseline ? genreSummaryWithTrackChange(next, baseline, updated) : next;
      }, current.summary);
      return { ...current, summary, highlights: current.highlights.map(project) };
    });
    setGenreAtlasGenres((current) => current.map((summary) => acceptedTracks.reduce((next, updated) => {
      const baseline = baselines.get(updated.trackKey);
      return baseline ? genreSummaryWithTrackChange(next, baseline, updated) : next;
    }, summary)));
    acceptedTracks.forEach((track) => playback.refreshTrack(track, true));
    setSnapshot((current) => {
      if (!current) return current;
      const deltas = acceptedTracks.reduce((totals, updated) => {
        const baseline = baselines.get(updated.trackKey);
        if (!baseline) return totals;
        totals.loved += Number(updated.loved) - Number(baseline.loved);
        totals.rated += Number(updated.rating !== null) - Number(baseline.rating !== null);
        return totals;
      }, { loved: 0, rated: 0 });
      return {
        ...current,
        summary: {
          ...current.summary,
          loved: Math.max(0, current.summary.loved + deltas.loved),
          rated: Math.max(0, current.summary.rated + deltas.rated),
        },
        tracks: current.tracks.map(project),
      };
    });
    return projection.complete;
  }

  async function refreshTagEditorCatalogViews(sync: CatalogSync) {
    // Verified file tags are available even when catalog import is still pending.
    setGenreIndexReloadToken((value) => value + 1);
    setGenreDetailReloadToken((value) => value + 1);
    // A save callback can outlive the selection that created it.
    const albumId = selectedAlbumIdRef.current;
    const selectionRequestId = albumRequestRef.current;
    const currentScroll = mainScrollRef.current?.scrollTop;
    if (typeof currentScroll === "number" && currentScroll > 0) {
      scrollPositionByDestinationRef.current[activeNav] = currentScroll;
    }
    const catalogRefreshed = await handleCatalogSync(sync, true);
    if (catalogRefreshed || !albumId) return;
    if (sync.status !== "synced" || selectedAlbumIdRef.current !== albumId
      || selectionRequestId !== albumRequestRef.current) return;

    const requestId = selectionRequestId;
    try {
      const detail = await loadAlbumDetail(albumId);
      if (requestId !== albumRequestRef.current || selectedAlbumIdRef.current !== albumId) return;
      const projectedAlbum = applyAlbumTrackMetricsProjection(detail.album, detail.tracks);
      setExplorerAlbums((current) => current.map((album) => album.id === albumId ? projectedAlbum : album));
      setAlbumTracks(applyAlbumPopularity(detail.tracks, detail.popularity));
      setAlbumTracksTruncated(detail.tracksTruncated);
      setSelectedTrack((current) => {
        if (!current || current.albumId !== albumId) return current;
        return detail.tracks.find((candidate) => candidate.trackKey === current.trackKey) ?? current;
      });
      setAlbumDetailState("ready");
    } catch (error) {
      console.warn("Aurora could not refresh the selected album after the tag edit", error);
    }
  }

  function applyTrackChange(updated: Track, previous?: Track, updateSelected = true) {
    const baseline = previous ?? (selectedTrack?.trackKey === updated.trackKey ? selectedTrack : undefined);
    const refreshMatchingTrack = (track: Track) => track.trackKey === updated.trackKey
      ? applyTrackTagProjection(track, updated)
      : track;
    if (updateSelected) {
      setSelectedTrack((current) => current ? refreshMatchingTrack(current) : current);
    }
    setExplorerTracks((current) => current.map(refreshMatchingTrack));
    if (updated.albumId && updated.albumId === selectedAlbumId) {
      const projectedTracks = albumTracks.map(refreshMatchingTrack);
      setExplorerAlbums((current) => current.map((album) => (
        album.id === updated.albumId
          ? applyAlbumTrackMetricsProjection(album, projectedTracks)
          : album
      )));
    }
    setAlbumTracks((current) => current.map(refreshMatchingTrack));
    setYearAlbumTracks((current) => current.map(refreshMatchingTrack));
    setRatingAlbumTracks((current) => current.map(refreshMatchingTrack));
    setPublisherAlbumTracks((current) => current.map(refreshMatchingTrack));
    setGenreDetail((current) => {
      if (!current) return current;
      return {
        ...current,
        summary: baseline ? genreSummaryWithTrackChange(current.summary, baseline, updated) : current.summary,
        highlights: current.highlights.map(refreshMatchingTrack),
      };
    });
    if (baseline) {
      setGenreAtlasGenres((current) => current.map((summary) => genreSummaryWithTrackChange(summary, baseline, updated)));
    }
    playback.refreshTrack(updated);
    setSnapshot((current) => {
      if (!current) return current;
      const lovedDelta = baseline ? Number(updated.loved) - Number(baseline.loved) : 0;
      const ratedDelta = baseline ? Number(updated.rating !== null) - Number(baseline.rating !== null) : 0;
      return {
        ...current,
        summary: {
          ...current.summary,
          loved: Math.max(0, current.summary.loved + lovedDelta),
          rated: Math.max(0, current.summary.rated + ratedDelta),
        },
        tracks: current.tracks.map(refreshMatchingTrack),
      };
    });
  }

  useLayoutEffect(() => {
    shortcutResultHandlerRef.current = (result) => {
      if (result.playback) playback.applySnapshot(result.playback);
      const projection = result.success && result.catalogSync
        ? acceptTrackProjectionKeys(
          result.track ? [result.track.trackKey] : [],
          result.catalogSync.projectionToken,
        )
        : null;
      if (result.track && projection && !projection.acceptedTrackKeys.has(result.track.trackKey)) return;
      setSyncMessage(result.success ? result.message : `Shortcut failed: ${result.message}`);
      if (result.track) applyTrackChange(result.track, result.previousTrack ?? undefined);
      if (result.success && result.catalogSync) {
        void handleCatalogSync(result.catalogSync, true);
      }
    };
  });

  async function saveInlineTagChange(track: Track, desired: TagValues) {
    if (inlineSaveRef.current.has(track.trackKey)) return;
    const expected = tagValuesForTrack(track);
    if (JSON.stringify(expected) === JSON.stringify(desired)) return;

    inlineSaveRef.current.add(track.trackKey);
    setInlineSavingKeys((current) => new Set(current).add(track.trackKey));
    setSyncMessage(null);

    const optimistic = trackWithTagValues(track, desired);
    const projectionTokenAtStart = latestTrackProjectionTokensRef.current.get(track.trackKey) ?? 0;
    applyTrackChange(optimistic, track, false);
    try {
      const snapshot = await updateTrackTags(track, expected, desired);
      const projection = acceptTrackProjectionKeys(
        [snapshot.track.trackKey],
        snapshot.catalogSync?.projectionToken,
      );
      if (!projection.acceptedTrackKeys.has(snapshot.track.trackKey)) return;
      applyTrackChange(snapshot.track, optimistic);
      await handleCatalogSync(snapshot.catalogSync, true);
      setInlineTagRevisions((current) => ({
        ...current,
        [track.trackKey]: (current[track.trackKey] ?? 0) + 1,
      }));
      if (snapshot.track.albumId && snapshot.track.albumId === selectedAlbumIdRef.current) {
        const albumId = snapshot.track.albumId;
        const requestId = albumRequestRef.current;
        try {
          const detail = await loadAlbumDetail(albumId);
          if (requestId === albumRequestRef.current && selectedAlbumIdRef.current === albumId) {
            const projectedAlbum = applyAlbumTrackMetricsProjection(detail.album, detail.tracks);
            setExplorerAlbums((current) => current.map((album) => album.id === albumId ? projectedAlbum : album));
            setAlbumTracks(applyAlbumPopularity(detail.tracks, detail.popularity));
            setAlbumTracksTruncated(detail.tracksTruncated);
            setSelectedTrack((current) => {
              if (!current || current.albumId !== albumId) return current;
              return detail.tracks.find((candidate) => candidate.trackKey === current.trackKey) ?? current;
            });
          }
        } catch (error) {
          console.warn("Aurora could not refresh the album rating after the track edit", error);
        }
      }
    } catch (error) {
      if ((latestTrackProjectionTokensRef.current.get(track.trackKey) ?? 0) === projectionTokenAtStart) {
        applyTrackChange(track, optimistic, false);
      }
      const message = error instanceof Error ? error.message : String(error);
      setSyncMessage(`Could not save ${track.title}: ${message}`);
    } finally {
      inlineSaveRef.current.delete(track.trackKey);
      setInlineSavingKeys((current) => {
        const next = new Set(current);
        next.delete(track.trackKey);
        return next;
      });
    }
  }

  return { inlineSavingKeys, inlineTagRevisions, applyTrackChanges, refreshTagEditorCatalogViews, saveInlineTagChange };
}
