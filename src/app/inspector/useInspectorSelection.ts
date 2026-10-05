import { useCallback, useEffect, useLayoutEffect, useRef, useState, type Dispatch, type RefObject, type SetStateAction } from "react";
import type { ExplorerAlbum, ExplorerLoadState } from "../../components/explorer/DeepExplorer";
import { transitionContent } from "../../contentTransition";
import {
  applyAlbumPopularity, applyAlbumTrackMetricsProjection, loadAlbumDetail, loadAlbumPopularity,
  type AlbumSummary, type Track,
} from "../../library";
import { shouldFollowPlaybackTransition, type usePlayback } from "../../playback";
import type { ViewPreferences } from "../../viewPreferences";

export type InspectorSelectionOptions = {
  initialViewPreferences: ViewPreferences;
  selectedTrack: Track | null;
  setSelectedTrack: Dispatch<SetStateAction<Track | null>>;
  artistRequestRef: RefObject<number>;
  playback: {
    state: Pick<ReturnType<typeof usePlayback>["state"], "currentTrack">;
    play: ReturnType<typeof usePlayback>["play"];
  };
  setExplorerAlbums: Dispatch<SetStateAction<AlbumSummary[]>>;
  endGenreQueue: () => void;
  setSyncMessage: Dispatch<SetStateAction<string | null>>;
};

/** Selection owns request generations so every destination invalidates the same work. */
export function useInspectorSelection({
  initialViewPreferences, selectedTrack, setSelectedTrack, artistRequestRef,
  playback, setExplorerAlbums, endGenreQueue, setSyncMessage,
}: InspectorSelectionOptions) {
  const [inspectorView, setInspectorView] = useState(initialViewPreferences.inspectorView);
  const [tagSelectionKind, setTagSelectionKind] = useState(initialViewPreferences.tagSelectionKind);
  const [selectedAlbumId, setSelectedAlbumIdState] = useState<string | null>(initialViewPreferences.selectedAlbumId);
  const [albumFileRefreshRequest, setAlbumFileRefreshRequest] = useState<{ albumId: string; requestId: number; } | null>(null);
  const [albumTracks, setAlbumTracks] = useState<Track[]>([]);
  const [albumTracksTruncated, setAlbumTracksTruncated] = useState(false);
  const [albumDetailState, setAlbumDetailState] = useState<ExplorerLoadState>("ready");
  const albumRequestRef = useRef(0);
  const selectedTrackRef = useRef<Track | null>(selectedTrack);
  const previousPlaybackTrackKeyRef = useRef<string | null>(null);
  const selectedAlbumIdRef = useRef<string | null>(selectedAlbumId);
  const inspectorViewRef = useRef(inspectorView);
  // Async completions must see navigation intent before its transition commits.
  const setSelectedAlbumId = useCallback((next: SetStateAction<string | null>) => {
    const albumId = typeof next === "function" ? next(selectedAlbumIdRef.current) : next;
    selectedAlbumIdRef.current = albumId;
    setSelectedAlbumIdState(albumId);
  }, []);
  // Forward actions through the latest ports without restarting file readback effects.
  const portsRef = useRef({ artistRequestRef, setSelectedTrack, setExplorerAlbums, endGenreQueue, setSyncMessage, playback });
  useLayoutEffect(() => {
    selectedTrackRef.current = selectedTrack;
    inspectorViewRef.current = inspectorView;
    portsRef.current = { artistRequestRef, setSelectedTrack, setExplorerAlbums, endGenreQueue, setSyncMessage, playback };
  });

  useEffect(() => {
    const currentTrack = playback.state.currentTrack;
    const previousTrackKey = previousPlaybackTrackKeyRef.current;
    previousPlaybackTrackKeyRef.current = currentTrack?.trackKey ?? null;
    if (
      !currentTrack
      || currentTrack.trackKey === previousTrackKey
      || !shouldFollowPlaybackTransition(previousTrackKey, selectedTrackRef.current?.trackKey ?? null, tagSelectionKind)
    ) return;
    portsRef.current.artistRequestRef.current += 1;
    portsRef.current.setSelectedTrack(currentTrack);
    setTagSelectionKind("track");
  }, [playback.state.currentTrack, tagSelectionKind]);

  const refreshSelectedAlbumPopularity = useCallback((albumId: string, requestId: number) => {
    void loadAlbumPopularity(albumId).then((popularity) => {
      if (requestId !== albumRequestRef.current) return;
      setAlbumTracks((current) => applyAlbumPopularity(current, popularity));
    }).catch(() => {
      // Cached evidence remains visible when Last.fm is offline or not configured.
    });
  }, []);

  const refreshSelectedAlbumFiles = useCallback((albumId: string, requestId: number) => {
    void loadAlbumDetail(albumId).then((detail) => {
      if (requestId !== albumRequestRef.current) return;
      const projectedAlbum = applyAlbumTrackMetricsProjection(detail.album, detail.tracks);
      portsRef.current.setExplorerAlbums((current) => current.map((album) => album.id === albumId ? projectedAlbum : album));
      setAlbumTracks(applyAlbumPopularity(detail.tracks, detail.popularity));
      setAlbumTracksTruncated(detail.tracksTruncated);
      portsRef.current.setSelectedTrack((current) => detail.tracks.find((track) => track.trackKey === current?.trackKey) ?? detail.tracks[0] ?? null);
    }).catch((error: unknown) => {
      // Keep the local tracks usable if the music share cannot be refreshed.
      console.warn("Aurora could not refresh album files", error);
    }).finally(() => {
      if (requestId === albumRequestRef.current) refreshSelectedAlbumPopularity(albumId, requestId);
    });
  }, [refreshSelectedAlbumPopularity]);

  // Read files only after the selected local detail has committed its transition.
  useEffect(() => {
    if (albumFileRefreshRequest && albumFileRefreshRequest.requestId === albumRequestRef.current) {
      refreshSelectedAlbumFiles(albumFileRefreshRequest.albumId, albumFileRefreshRequest.requestId);
    }
  }, [albumFileRefreshRequest, refreshSelectedAlbumFiles]);

  const selectTrack = useCallback((track: Track) => {
    portsRef.current.artistRequestRef.current += 1;
    portsRef.current.setSelectedTrack(track);
    setTagSelectionKind("track");
    if (inspectorViewRef.current !== "tags") setInspectorView("track");
  }, []);

  const selectAlbum = useCallback((album: ExplorerAlbum | null) => {
    transitionContent(() => {
      const requestId = ++albumRequestRef.current;
      portsRef.current.artistRequestRef.current += 1;
      setSelectedAlbumId(album?.id ?? null);
      setAlbumTracks([]);
      setAlbumTracksTruncated(false);
      if (!album) {
        setAlbumDetailState("ready");
        setInspectorView("track");
        setTagSelectionKind("track");
        return;
      }
      portsRef.current.setSelectedTrack(null);
      setTagSelectionKind("album");
      if (inspectorViewRef.current !== "tags") setInspectorView("album");
      setAlbumDetailState("loading");
      void loadAlbumDetail(album.id, { localOnly: true })
        .then((detail) => {
          transitionContent(() => {
            if (requestId !== albumRequestRef.current) return;
            const projectedAlbum = applyAlbumTrackMetricsProjection(detail.album, detail.tracks);
            portsRef.current.setExplorerAlbums((current) => current.map((candidate) => candidate.id === detail.album.id ? projectedAlbum : candidate));
            setAlbumTracks(applyAlbumPopularity(detail.tracks, detail.popularity));
            setAlbumTracksTruncated(detail.tracksTruncated);
            portsRef.current.setSelectedTrack(detail.tracks[0] ?? null);
            setAlbumDetailState("ready");
            setAlbumFileRefreshRequest({ albumId: album.id, requestId });
          }, "album-detail");
        })
        .catch((error: unknown) => {
          if (requestId !== albumRequestRef.current) return;
          console.warn("Aurora could not open album details", error);
          setAlbumDetailState("error");
        });
    }, "album-detail");
  }, [setSelectedAlbumId]);

  async function playExplorerAlbum(album: ExplorerAlbum) {
    try {
      const tracks = selectedAlbumId === album.id && albumTracks.length > 0
        ? albumTracks
        : (await loadAlbumDetail(album.id)).tracks;
      if (tracks.length === 0) {
        portsRef.current.setSyncMessage(`${album.title} has no playable tracks in the bounded album detail.`);
        return;
      }
      portsRef.current.endGenreQueue();
      const next = await portsRef.current.playback.play(tracks, tracks[0].id);
      if (next) selectTrack(tracks[0]);
    } catch (error) {
      portsRef.current.setSyncMessage(`Could not play ${album.title}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  return {
    inspectorView, setInspectorView, inspectorViewRef, tagSelectionKind, setTagSelectionKind,
    selectedAlbumId, setSelectedAlbumId, selectedAlbumIdRef, selectedTrackRef,
    albumTracks, setAlbumTracks, albumTracksTruncated, setAlbumTracksTruncated,
    albumDetailState, setAlbumDetailState, albumFileRefreshRequest, setAlbumFileRefreshRequest,
    albumRequestRef, refreshSelectedAlbumFiles, selectTrack, selectAlbum, playExplorerAlbum,
  };
}

export type InspectorSelection = ReturnType<typeof useInspectorSelection>;
