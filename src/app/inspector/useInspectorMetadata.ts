import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { loadCatalogChartRankings, type CatalogChartRankings } from "../../charts";
import type { ExplorerView } from "../../components/explorer/DeepExplorer";
import type { GenreDetail } from "../../genres";
import { loadTrackHistoryInsight, type TrackHistoryInsight } from "../../history";
import { subscribeNativeEvent } from "../../nativeEvents";
import type { AlbumSummary, LibrarySnapshot, Track } from "../../library";
import type { YearAlbum } from "../../years";

export type InspectorMetadataOptions = {
  selectedTrack: Track | null;
  inspectorTrack: Track | null;
  setSelectedTrack: Dispatch<SetStateAction<Track | null>>;
  selectedAlbumId: string | null;
  albumTracks: Track[];
  explorerView: ExplorerView;
  explorerTracks: Track[];
  explorerAlbums: AlbumSummary[];
  selectedYearAlbum: YearAlbum | null;
  snapshot: LibrarySnapshot | null;
  yearAlbumTracks: Track[];
  ratingAlbumTracks: Track[];
  publisherAlbumTracks: Track[];
  genreDetail: GenreDetail | null;
};

/** Reconciles committed destination data without coupling their loading lifecycles. */
export function useInspectorMetadata({
  selectedTrack, inspectorTrack, setSelectedTrack, selectedAlbumId, albumTracks, explorerView,
  explorerTracks, explorerAlbums, selectedYearAlbum, snapshot, yearAlbumTracks,
  ratingAlbumTracks, publisherAlbumTracks, genreDetail,
}: InspectorMetadataOptions) {
  const [trackHistory, setTrackHistory] = useState<{ trackKey: string; value: TrackHistoryInsight; } | null>(null);
  const [catalogChartRanks, setCatalogChartRanks] = useState<CatalogChartRankings>({ tracks: {}, albums: {} });
  const inspectorTrackId = inspectorTrack?.id;

  useEffect(() => {
    const visibleTracks = explorerView === "tracks" ? explorerTracks : albumTracks;
    const trackIds = [...new Set([
      ...(inspectorTrackId ? [inspectorTrackId] : []),
      ...(selectedTrack ? [selectedTrack.id] : []),
      ...visibleTracks.map((track) => track.id),
    ])].slice(0, 100);
    const albumIds = [...new Set([
      ...explorerAlbums.map((album) => album.id),
      ...(selectedAlbumId ? [selectedAlbumId] : []),
      ...(selectedYearAlbum ? [selectedYearAlbum.id] : []),
    ])].slice(0, 200 - trackIds.length);
    if (trackIds.length === 0 && albumIds.length === 0) return;
    let cancelled = false;
    void loadCatalogChartRankings({ trackIds, albumIds })
      .then((rankings) => {
        if (!cancelled) setCatalogChartRanks(rankings);
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setCatalogChartRanks({ tracks: {}, albums: {} });
        console.warn("Aurora could not load Music Library chart rankings", error);
      });
    return () => { cancelled = true; };
  }, [albumTracks, explorerAlbums, explorerTracks, explorerView, inspectorTrackId, selectedAlbumId, selectedTrack, selectedYearAlbum]);

  useEffect(() => {
    const trackKey = inspectorTrack?.trackKey;
    if (!trackKey) return;
    let cancelled = false;
    const refresh = () => {
      void loadTrackHistoryInsight(trackKey)
        .then((value) => {
          if (!cancelled) setTrackHistory({ trackKey, value });
        })
        .catch(() => undefined);
    };
    const stop = subscribeNativeEvent<string>("history://revision", refresh, refresh);
    return () => {
      cancelled = true;
      stop();
    };
  }, [inspectorTrack?.trackKey]);

  useEffect(() => {
    const candidates = [
      ...(snapshot?.tracks ?? []), ...explorerTracks, ...albumTracks,
      ...yearAlbumTracks, ...ratingAlbumTracks, ...publisherAlbumTracks,
      ...(genreDetail?.highlights ?? []),
    ];
    if (candidates.length === 0) return;
    const timer = window.setTimeout(() => {
      setSelectedTrack((current) => {
        if (!current) return current;
        return candidates.find((track) => track.trackKey === current.trackKey) ?? current;
      });
    }, 0);
    return () => window.clearTimeout(timer);
  }, [snapshot, explorerTracks, albumTracks, yearAlbumTracks, ratingAlbumTracks, publisherAlbumTracks, genreDetail, setSelectedTrack]);

  return { trackHistory, catalogChartRanks };
}
