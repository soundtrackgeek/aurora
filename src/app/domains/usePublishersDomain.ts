import { type Dispatch, type RefObject, type SetStateAction, useEffect, useLayoutEffect, useRef, useState } from "react";
import { type SidebarDestination } from "../../components/navigation/SidebarNavigation";
import { type PublisherLoadState } from "../../components/publishers/PublisherSignalTimeline";
import { transitionContent } from "../../contentTransition";
import { formatCount, type Track } from "../../library";
import { usePlayback } from "../../playback";
import {
  loadPublisherAlbumTracks,
  loadPublisherDetail,
  loadPublisherOverview,
  loadPublisherQueue,
  type PublisherAlbum,
  type PublisherDetail,
  type PublisherOverview,
  type PublisherSummary
} from "../../publishers";
import { type InspectorView, type TagSelectionKind } from "../../viewPreferences";

interface PublishersDomainOptions {
  setInspectorView: Dispatch<SetStateAction<InspectorView>>;
  setTagSelectionKind: Dispatch<SetStateAction<TagSelectionKind>>;
  activeNav: SidebarDestination;
  loadedPageRequestsRef: RefObject<Map<string, string>>;
  inspectorViewRef: RefObject<InspectorView>;
  playback: Pick<ReturnType<typeof usePlayback>, "play">;
  libraryReady: boolean;
  endGenreQueue: () => void;
  selectTrack: (track: Track) => void;
}

/** Owns the publishers destination's state, request guards, and actions. */
export function usePublishersDomain({
  setInspectorView,
  setTagSelectionKind,
  activeNav,
  loadedPageRequestsRef,
  inspectorViewRef,
  playback,
  libraryReady,
  endGenreQueue,
  selectTrack,
}: PublishersDomainOptions) {
  const [publisherOverview, setPublisherOverview] = useState<PublisherOverview | null>(null);
  const [publisherDetail, setPublisherDetail] = useState<PublisherDetail | null>(null);
  const [publisherLoadState, setPublisherLoadState] = useState<PublisherLoadState>("loading");
  const [publisherDetailState, setPublisherDetailState] = useState<PublisherLoadState>("loading");
  const [publisherError, setPublisherError] = useState<string | null>(null);
  const [publisherDetailError, setPublisherDetailError] = useState<string | null>(null);
  const [publisherSearch, setPublisherSearch] = useState("");
  const [publisherReloadToken, setPublisherReloadToken] = useState(0);
  const [publisherQueueBusy, setPublisherQueueBusy] = useState(false);
  const [publisherQueueMessage, setPublisherQueueMessage] = useState<string | null>(null);
  const [selectedPublisherAlbum, setSelectedPublisherAlbum] = useState<PublisherAlbum | null>(null);
  const [publisherAlbumTracks, setPublisherAlbumTracks] = useState<Track[]>([]);
  const [publisherAlbumBusy, setPublisherAlbumBusy] = useState(false);
  const publisherOverviewRequestRef = useRef(0);
  const publisherDetailRequestRef = useRef(0);
  const publisherAlbumRequestRef = useRef(0);
  const publisherLoadedSearchRef = useRef<string | null>(null);
  const publisherDetailRef = useRef<PublisherDetail | null>(publisherDetail);
  const selectedPublisherAlbumRef = useRef<PublisherAlbum | null>(selectedPublisherAlbum);

  useLayoutEffect(() => { publisherDetailRef.current = publisherDetail; }, [publisherDetail]);

  useLayoutEffect(() => { selectedPublisherAlbumRef.current = selectedPublisherAlbum; }, [selectedPublisherAlbum]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Publishers") return;
    const pageRequestKey = JSON.stringify([publisherSearch, publisherReloadToken]);
    if (loadedPageRequestsRef.current.get("publishers") === pageRequestKey) return;
    const requestId = ++publisherOverviewRequestRef.current;
    publisherDetailRequestRef.current += 1;
    publisherAlbumRequestRef.current += 1;
    const preserveSelection = publisherLoadedSearchRef.current === publisherSearch;
    const previousPublisher = preserveSelection
      ? publisherDetailRef.current?.publisher.name ?? null
      : null;
    const previousAlbumId = preserveSelection
      ? selectedPublisherAlbumRef.current?.id ?? null
      : null;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setPublisherLoadState("loading");
      setPublisherDetailState("loading");
      setPublisherError(null);
      setPublisherDetailError(null);
      setPublisherQueueMessage(null);
      const detailRequest = previousPublisher
        ? loadPublisherDetail(previousPublisher).catch(() => null)
        : Promise.resolve(null);
      void Promise.all([loadPublisherOverview(publisherSearch), detailRequest])
        .then(([overview, refreshedDetail]) => {
          transitionContent(() => {
            if (cancelled || requestId !== publisherOverviewRequestRef.current) return;
            const detail = refreshedDetail ?? overview.initialDetail;
            publisherLoadedSearchRef.current = publisherSearch;
            loadedPageRequestsRef.current.set("publishers", pageRequestKey);
            setPublisherOverview(overview);
            setPublisherDetail(detail);
            setPublisherLoadState("ready");
            setPublisherDetailState("ready");
            const initialAlbum = detail.albums.find((album) => album.id === previousAlbumId)
              ?? detail.albums[0]
              ?? null;
            setSelectedPublisherAlbum(initialAlbum);
            setPublisherAlbumTracks([]);
            if (!initialAlbum) return;
            if (!preserveSelection) setInspectorView("album");
            const albumRequestId = ++publisherAlbumRequestRef.current;
            void loadPublisherAlbumTracks(initialAlbum)
              .then((tracks) => {
                transitionContent(() => {
                  if (!cancelled && albumRequestId === publisherAlbumRequestRef.current) {
                    setPublisherAlbumTracks(tracks);
                  }
                }, "collection");
              })
              .catch(() => undefined);
          }, "collection");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== publisherOverviewRequestRef.current) return;
          setPublisherError(error instanceof Error ? error.message : String(error));
          setPublisherLoadState("error");
          setPublisherDetailState("error");
        });
    }, publisherSearch.trim() ? 160 : 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, publisherReloadToken, publisherSearch, loadedPageRequestsRef, setInspectorView]);

  function selectPublisher(publisher: PublisherSummary) {
    transitionContent(() => {
      const requestId = ++publisherDetailRequestRef.current;
      publisherAlbumRequestRef.current += 1;
      setPublisherDetailState("loading");
      setPublisherDetailError(null);
      setPublisherQueueMessage(null);
      void loadPublisherDetail(publisher.name)
        .then((detail) => {
          transitionContent(() => {
            if (requestId !== publisherDetailRequestRef.current) return;
            setPublisherDetail(detail);
            setPublisherDetailState("ready");
            const initialAlbum = detail.albums[0] ?? null;
            setSelectedPublisherAlbum(initialAlbum);
            setPublisherAlbumTracks([]);
            if (initialAlbum) openPublisherAlbum(initialAlbum);
          }, "collection");
        })
        .catch((error: unknown) => {
          if (requestId !== publisherDetailRequestRef.current) return;
          setPublisherDetailError(error instanceof Error ? error.message : String(error));
          setPublisherDetailState("error");
        });
    }, "collection");
  }

  function openPublisherAlbum(album: PublisherAlbum) {
    transitionContent(() => {
      const requestId = ++publisherAlbumRequestRef.current;
      setSelectedPublisherAlbum(album);
      setTagSelectionKind("album");
      if (inspectorViewRef.current !== "tags") setInspectorView("album");
      setPublisherAlbumTracks([]);
      void loadPublisherAlbumTracks(album)
        .then((tracks) => {
          transitionContent(() => {
            if (requestId === publisherAlbumRequestRef.current) setPublisherAlbumTracks(tracks);
          }, "collection");
        })
        .catch((error: unknown) => {
          if (requestId === publisherAlbumRequestRef.current) {
            setPublisherDetailError(error instanceof Error ? error.message : String(error));
          }
        });
    }, "collection");
  }

  async function playPublisher(publisher: string) {
    if (publisherQueueBusy) return;
    setPublisherQueueBusy(true);
    setPublisherQueueMessage(null);
    try {
      const tracks = await loadPublisherQueue(publisher, 100);
      if (!tracks.length) {
        setPublisherQueueMessage("No playable tracks were found for this publisher.");
        return;
      }
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) {
        selectTrack(tracks[0]);
        setPublisherQueueMessage(`Loaded ${formatCount(tracks.length)} tracks from ${publisher}.`);
      }
    } catch (error) {
      setPublisherQueueMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setPublisherQueueBusy(false);
    }
  }

  async function playPublisherAlbum(album: PublisherAlbum) {
    if (publisherAlbumBusy) return;
    setPublisherAlbumBusy(true);
    try {
      const tracks = selectedPublisherAlbum?.id === album.id && publisherAlbumTracks.length
        ? publisherAlbumTracks
        : await loadPublisherAlbumTracks(album);
      if (!tracks.length) {
        setPublisherQueueMessage(`${album.title} has no playable tracks.`);
        return;
      }
      setPublisherAlbumTracks(tracks);
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) selectTrack(tracks[0]);
    } catch (error) {
      setPublisherQueueMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setPublisherAlbumBusy(false);
    }
  }

  return {
    publisherOverview,
    publisherDetail,
    publisherLoadState,
    publisherDetailState,
    publisherError,
    publisherDetailError,
    publisherSearch,
    setPublisherSearch,
    setPublisherReloadToken,
    publisherQueueBusy,
    publisherQueueMessage,
    selectedPublisherAlbum,
    publisherAlbumTracks,
    setPublisherAlbumTracks,
    publisherAlbumBusy,
    selectPublisher,
    openPublisherAlbum,
    playPublisher,
    playPublisherAlbum,
  };
}
