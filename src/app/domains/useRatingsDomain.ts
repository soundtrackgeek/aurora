import { useEffect, useLayoutEffect, useRef, useState, type Dispatch, type RefObject, type SetStateAction } from "react";
import { type SidebarDestination } from "../../components/navigation/SidebarNavigation";
import { type RatingsLoadState } from "../../components/ratings/RatingsStudio";
import { transitionContent } from "../../contentTransition";
import { formatCount, type Track } from "../../library";
import { usePlayback } from "../../playback";
import {
  loadRatingAlbumPage,
  loadRatingAlbumQueue,
  loadRatingAlbumTracks,
  loadRatingCollection,
  loadRatingsOverview,
  type CompletionKind,
  type RatingAlbum,
  type RatingAlbumPage,
  type RatingMode,
  type RatingsOverview
} from "../../ratings";
import { loadTonightQueue } from "../../tonight";
import { shouldRetargetTagsForAlbumSelection, type InspectorView, type TagSelectionKind } from "../../viewPreferences";

interface RatingsDomainOptions {
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

/** Owns the ratings destination's state, request guards, and actions. */
export function useRatingsDomain({
  setInspectorView,
  setTagSelectionKind,
  activeNav,
  loadedPageRequestsRef,
  inspectorViewRef,
  playback,
  libraryReady,
  endGenreQueue,
  selectTrack,
}: RatingsDomainOptions) {
  const [ratingsOverview, setRatingsOverview] = useState<RatingsOverview | null>(null);
  const [ratingsPage, setRatingsPage] = useState<RatingAlbumPage | null>(null);
  const [ratingsLoadState, setRatingsLoadState] = useState<RatingsLoadState>("loading");
  const [ratingsPageState, setRatingsPageState] = useState<RatingsLoadState>("loading");
  const [ratingsError, setRatingsError] = useState<string | null>(null);
  const [ratingsPageError, setRatingsPageError] = useState<string | null>(null);
  const [ratingsReloadToken, setRatingsReloadToken] = useState(0);
  const [ratingsRefreshing, setRatingsRefreshing] = useState(false);
  const [ratingsCompletion, setRatingsCompletion] = useState<CompletionKind>("partiallyRated");
  const [ratingsRemainingTracks, setRatingsRemainingTracks] = useState<number | null>(null);
  const [selectedRatingAlbum, setSelectedRatingAlbum] = useState<RatingAlbum | null>(null);
  const [ratingAlbumTracks, setRatingAlbumTracks] = useState<Track[]>([]);
  const [ratingsQueueBusy, setRatingsQueueBusy] = useState(false);
  const [ratingsQueueMessage, setRatingsQueueMessage] = useState<string | null>(null);
  const ratingsRequestRef = useRef(0);
  const ratingsPageRequestRef = useRef(0);
  const ratingsAlbumRequestRef = useRef(0);
  const ratingsLoadedTokenRef = useRef(-1);
  const ratingsPreserveInspectorTokenRef = useRef<number | null>(null);
  const selectedRatingAlbumRef = useRef<RatingAlbum | null>(selectedRatingAlbum);

  useLayoutEffect(() => { selectedRatingAlbumRef.current = selectedRatingAlbum; }, [selectedRatingAlbum]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Ratings") return;
    const pageRequestKey = String(ratingsReloadToken);
    if (loadedPageRequestsRef.current.get("ratings-overview") === pageRequestKey) return;
    const requestId = ++ratingsRequestRef.current;
    ratingsPageRequestRef.current += 1;
    ratingsAlbumRequestRef.current += 1;
    const preserveSelection = ratingsLoadedTokenRef.current >= 0;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (!preserveSelection) {
        setRatingsLoadState("loading");
        setRatingsPageState("loading");
        setRatingsPage(null);
      }
      setRatingsError(null);
      setRatingsPageError(null);
      setRatingsQueueMessage(null);
      void loadRatingsOverview()
        .then((overview) => {
          transitionContent(() => {
            if (cancelled || requestId !== ratingsRequestRef.current) return;
            loadedPageRequestsRef.current.set("ratings-overview", pageRequestKey);
            setRatingsOverview(overview);
            ratingsPreserveInspectorTokenRef.current = preserveSelection
              ? ratingsReloadToken
              : null;
            ratingsLoadedTokenRef.current = ratingsReloadToken;
            setRatingsLoadState("ready");
          }, "collection");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== ratingsRequestRef.current) return;
          setRatingsError(error instanceof Error ? error.message : String(error));
          setRatingsLoadState("error");
          setRatingsPageState("error");
          setRatingsRefreshing(false);
        });
    }, 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, ratingsReloadToken, loadedPageRequestsRef]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Ratings" || !ratingsOverview) return;
    if (ratingsLoadedTokenRef.current !== ratingsReloadToken) return;
    const pageRequestKey = JSON.stringify([ratingsCompletion, ratingsRemainingTracks, ratingsReloadToken]);
    if (loadedPageRequestsRef.current.get("ratings-page") === pageRequestKey) return;
    const pageRequestId = ++ratingsPageRequestRef.current;
    ratingsAlbumRequestRef.current += 1;
    const preserveSelection = ratingsPreserveInspectorTokenRef.current === ratingsReloadToken;
    const previousAlbumId = preserveSelection ? selectedRatingAlbumRef.current?.id ?? null : null;
    let cancelled = false;
    if (!preserveSelection) setRatingsPageState("loading");
    setRatingsPageError(null);
    const request = ratingsCompletion === "partiallyRated" && ratingsRemainingTracks === null
      ? Promise.resolve(ratingsOverview.initialPage)
      : loadRatingAlbumPage(ratingsCompletion, ratingsCompletion === "partiallyRated" ? ratingsRemainingTracks : null);
    void request
      .then((page) => {
        transitionContent(() => {
          if (cancelled || pageRequestId !== ratingsPageRequestRef.current) return;
          loadedPageRequestsRef.current.set("ratings-page", pageRequestKey);
          setRatingsPage(page);
          setRatingsPageState("ready");
          setRatingsRefreshing(false);
          const initialAlbum = page.albums.find((album) => album.id === previousAlbumId)
            ?? page.albums[0]
            ?? null;
          setSelectedRatingAlbum(initialAlbum);
          setRatingAlbumTracks([]);
          ratingsPreserveInspectorTokenRef.current = null;
          if (!initialAlbum) return;
          if (!preserveSelection) setInspectorView("album");
          const albumRequestId = ++ratingsAlbumRequestRef.current;
          void loadRatingAlbumTracks(initialAlbum)
            .then((tracks) => {
              transitionContent(() => {
                if (!cancelled && albumRequestId === ratingsAlbumRequestRef.current) {
                  setRatingAlbumTracks(tracks);
                }
              }, "collection");
            })
            .catch(() => undefined);
        }, "collection");
      })
      .catch((error: unknown) => {
        if (cancelled || pageRequestId !== ratingsPageRequestRef.current) return;
        setRatingsPageError(error instanceof Error ? error.message : String(error));
        setRatingsPageState("error");
        setRatingsRefreshing(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeNav, libraryReady, ratingsCompletion, ratingsOverview, ratingsReloadToken, ratingsRemainingTracks, loadedPageRequestsRef, setInspectorView]);

  function openRatingAlbum(album: RatingAlbum) {
    transitionContent(() => {
      const requestId = ++ratingsAlbumRequestRef.current;
      setSelectedRatingAlbum(album);
      if (shouldRetargetTagsForAlbumSelection(inspectorViewRef.current)) {
        setTagSelectionKind("album");
        setInspectorView("album");
      }
      setRatingAlbumTracks([]);
      void loadRatingAlbumTracks(album)
        .then((tracks) => {
          transitionContent(() => {
            if (requestId === ratingsAlbumRequestRef.current) setRatingAlbumTracks(tracks);
          }, "collection");
        })
        .catch((error: unknown) => {
          if (requestId === ratingsAlbumRequestRef.current) {
            setRatingsPageError(error instanceof Error ? error.message : String(error));
          }
        });
    }, "collection");
  }

  async function playRatingCollection(mode: RatingMode, rating: number | null) {
    if (ratingsQueueBusy) return;
    setRatingsQueueBusy(true);
    setRatingsQueueMessage(null);
    try {
      const tracks = await loadRatingCollection(mode, rating, 100);
      if (!tracks.length) {
        setRatingsQueueMessage("No playable tracks matched this rating band.");
        return;
      }
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) {
        selectTrack(tracks[0]);
        setRatingsQueueMessage(`Loaded ${formatCount(tracks.length)} tracks from this ${mode === "tracks" ? "track" : "album"} rating band.`);
      }
    } catch (error) {
      setRatingsQueueMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setRatingsQueueBusy(false);
    }
  }

  async function playRatingAlbumUnrated(album: RatingAlbum) {
    if (ratingsQueueBusy) return;
    setRatingsQueueBusy(true);
    setRatingsQueueMessage(null);
    try {
      const tracks = await loadRatingAlbumQueue(album, true, 100);
      if (!tracks.length) {
        setRatingsQueueMessage(`${album.title} has no unrated tracks left.`);
        return;
      }
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (next) {
        selectTrack(tracks[0]);
        setRatingsQueueMessage(`Loaded ${formatCount(tracks.length)} unrated tracks from ${album.title}.`);
      }
    } catch (error) {
      setRatingsQueueMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setRatingsQueueBusy(false);
    }
  }

  async function playTonightAlbum(album: RatingAlbum, minutes: number) {
    if (ratingsQueueBusy) throw new Error("Album playback is already loading.");
    setRatingsQueueBusy(true);
    try {
      const tracks = await loadTonightQueue(album, minutes);
      if (!tracks.length) throw new Error("This album has no playable tracks. Request fresh suggestions.");
      endGenreQueue();
      const next = await playback.play(tracks, tracks[0].id);
      if (!next) throw new Error("Album playback could not start. Check the player status.");
      selectTrack(tracks[0]);
    } finally { setRatingsQueueBusy(false); }
  }

  return {
    ratingsOverview,
    ratingsPage,
    ratingsLoadState,
    ratingsPageState,
    ratingsError,
    ratingsPageError,
    setRatingsReloadToken,
    ratingsRefreshing,
    setRatingsRefreshing,
    setRatingsCompletion,
    ratingsRemainingTracks,
    setRatingsRemainingTracks,
    selectedRatingAlbum,
    ratingAlbumTracks,
    setRatingAlbumTracks,
    ratingsQueueBusy,
    ratingsQueueMessage,
    openRatingAlbum,
    playRatingCollection,
    playRatingAlbumUnrated,
    playTonightAlbum,
  };
}
