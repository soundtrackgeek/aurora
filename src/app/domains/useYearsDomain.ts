import { type Dispatch, type RefObject, type SetStateAction, useEffect, useLayoutEffect, useRef, useState } from "react";
import { type YearsLoadState } from "../../components/library/YearsExplorer";
import { type SidebarDestination } from "../../components/navigation/SidebarNavigation";
import { transitionContent } from "../../contentTransition";
import { formatCount, type Track } from "../../library";
import { usePlayback } from "../../playback";
import { type InspectorView, type TagSelectionKind } from "../../viewPreferences";
import {
  loadYearAlbumTracks,
  loadYearDetail,
  loadYearOverview,
  loadYearQueue,
  type YearAlbum,
  type YearDetail,
  type YearOverview,
  type YearSelection
} from "../../years";

interface YearsDomainOptions {
  setInspectorView: Dispatch<SetStateAction<InspectorView>>;
  setTagSelectionKind: Dispatch<SetStateAction<TagSelectionKind>>;
  activeNav: SidebarDestination;
  inspectorViewRef: RefObject<InspectorView>;
  playback: Pick<ReturnType<typeof usePlayback>, "play">;
  libraryReady: boolean;
  endGenreQueue: () => void;
  selectTrack: (track: Track) => void;
}

/** Owns the years destination's state, request guards, and actions. */
export function useYearsDomain({
  setInspectorView,
  setTagSelectionKind,
  activeNav,
  inspectorViewRef,
  playback,
  libraryReady,
  endGenreQueue,
  selectTrack,
}: YearsDomainOptions) {
  const [yearOverview, setYearOverview] = useState<YearOverview | null>(null);
  const [yearDetail, setYearDetail] = useState<YearDetail | null>(null);
  const [yearLoadState, setYearLoadState] = useState<YearsLoadState>("loading");
  const [yearDetailState, setYearDetailState] = useState<YearsLoadState>("loading");
  const [yearError, setYearError] = useState<string | null>(null);
  const [yearDetailError, setYearDetailError] = useState<string | null>(null);
  const [yearReloadToken, setYearReloadToken] = useState(0);
  const [yearQueueBusy, setYearQueueBusy] = useState(false);
  const [yearQueueMessage, setYearQueueMessage] = useState<string | null>(null);
  const [selectedYearAlbum, setSelectedYearAlbum] = useState<YearAlbum | null>(null);
  const [yearAlbumTracks, setYearAlbumTracks] = useState<Track[]>([]);
  const [yearAlbumBusy, setYearAlbumBusy] = useState(false);
  const yearOverviewRequestRef = useRef(0);
  const yearDetailRequestRef = useRef(0);
  const yearAlbumRequestRef = useRef(0);
  const yearLoadedTokenRef = useRef(-1);
  const yearDetailRef = useRef<YearDetail | null>(yearDetail);
  const selectedYearAlbumRef = useRef<YearAlbum | null>(selectedYearAlbum);

  useLayoutEffect(() => { yearDetailRef.current = yearDetail; }, [yearDetail]);

  useLayoutEffect(() => { selectedYearAlbumRef.current = selectedYearAlbum; }, [selectedYearAlbum]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Years") return;
    if (yearOverview && yearLoadedTokenRef.current === yearReloadToken) {
      setYearLoadState("ready");
      return;
    }
    const requestId = ++yearOverviewRequestRef.current;
    yearDetailRequestRef.current += 1;
    const preserveSelection = yearLoadedTokenRef.current >= 0;
    const previousSelection = preserveSelection ? yearDetailRef.current?.selection ?? null : null;
    const previousAlbumId = preserveSelection ? selectedYearAlbumRef.current?.id ?? null : null;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setYearLoadState("loading");
      setYearDetailState("loading");
      setYearError(null);
      setYearDetailError(null);
      setYearQueueMessage(null);
      const detailRequest = previousSelection
        ? loadYearDetail(previousSelection).catch(() => null)
        : Promise.resolve(null);
      void Promise.all([loadYearOverview(), detailRequest])
        .then(([overview, refreshedDetail]) => {
          transitionContent(() => {
            if (cancelled || requestId !== yearOverviewRequestRef.current) return;
            const detail = refreshedDetail ?? overview.initialDetail;
            setYearOverview(overview);
            yearLoadedTokenRef.current = yearReloadToken;
            setYearDetail(detail);
            setYearLoadState("ready");
            setYearDetailState("ready");
            const initialAlbum = detail.albums.find((album) => album.id === previousAlbumId)
              ?? detail.albums[0]
              ?? null;
            setSelectedYearAlbum(initialAlbum);
            setYearAlbumTracks([]);
            if (!initialAlbum) return;
            if (!preserveSelection) setInspectorView("album");
            const albumRequestId = ++yearAlbumRequestRef.current;
            void loadYearAlbumTracks(initialAlbum)
              .then((tracks) => {
                transitionContent(() => {
                  if (albumRequestId === yearAlbumRequestRef.current) setYearAlbumTracks(tracks);
                }, "collection");
              })
              .catch(() => undefined);
          }, "collection");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== yearOverviewRequestRef.current) return;
          setYearError(error instanceof Error ? error.message : String(error));
          setYearLoadState("error");
          setYearDetailState("error");
        });
    }, 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, yearOverview, yearReloadToken, setInspectorView]);

  function selectYear(selection: YearSelection) {
    transitionContent(() => {
      const requestId = ++yearDetailRequestRef.current;
      setYearDetailState("loading");
      setYearDetailError(null);
      setYearQueueMessage(null);
      void loadYearDetail(selection)
        .then((detail) => {
          transitionContent(() => {
            if (requestId !== yearDetailRequestRef.current) return;
            setYearDetail(detail);
            setYearDetailState("ready");
            const nextAlbum = detail.albums[0] ?? null;
            setSelectedYearAlbum(nextAlbum);
            setYearAlbumTracks([]);
            if (nextAlbum) openYearAlbum(nextAlbum);
          }, "collection");
        })
        .catch((error: unknown) => {
          if (requestId !== yearDetailRequestRef.current) return;
          setYearDetailError(error instanceof Error ? error.message : String(error));
          setYearDetailState("error");
        });
    }, "collection");
  }

  function openYearAlbum(album: YearAlbum) {
    transitionContent(() => {
      const requestId = ++yearAlbumRequestRef.current;
      setSelectedYearAlbum(album);
      setYearAlbumTracks([]);
      setTagSelectionKind("album");
      if (inspectorViewRef.current !== "tags") setInspectorView("album");
      void loadYearAlbumTracks(album)
        .then((tracks) => {
          transitionContent(() => {
            if (requestId === yearAlbumRequestRef.current) setYearAlbumTracks(tracks);
          }, "collection");
        })
        .catch((error: unknown) => {
          if (requestId === yearAlbumRequestRef.current) {
            console.warn("Aurora could not open this year edition", error);
          }
        });
    }, "collection");
  }

  async function playYear(selection: YearSelection) {
    if (yearQueueBusy) return;
    setYearQueueBusy(true);
    setYearQueueMessage(null);
    try {
      const tracks = await loadYearQueue(selection, 100);
      if (tracks.length === 0) {
        setYearQueueMessage("No playable tracks were found for this clock selection.");
        return;
      }
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) {
        selectTrack(tracks[0]);
        setYearQueueMessage(`Loaded ${formatCount(tracks.length)} tracks from this ${selection.basis} year.`);
      }
    } catch (error) {
      setYearQueueMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setYearQueueBusy(false);
    }
  }

  async function playYearAlbum(album: YearAlbum) {
    if (yearAlbumBusy) return;
    setYearAlbumBusy(true);
    try {
      const tracks = selectedYearAlbum?.id === album.id && yearAlbumTracks.length
        ? yearAlbumTracks
        : await loadYearAlbumTracks(album);
      if (!tracks.length) {
        setYearQueueMessage(`${album.title} has no playable tracks in the bounded album detail.`);
        return;
      }
      setYearAlbumTracks(tracks);
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) selectTrack(tracks[0]);
    } catch (error) {
      setYearQueueMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setYearAlbumBusy(false);
    }
  }

  return {
    yearOverview,
    yearDetail,
    yearLoadState,
    yearDetailState,
    yearError,
    yearDetailError,
    setYearReloadToken,
    yearQueueBusy,
    yearQueueMessage,
    selectedYearAlbum,
    yearAlbumTracks,
    setYearAlbumTracks,
    yearAlbumBusy,
    selectYear,
    openYearAlbum,
    playYear,
    playYearAlbum,
  };
}
