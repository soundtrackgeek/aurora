import { type RefObject, useEffect, useLayoutEffect, useRef, useState } from "react";
import { type GenreAtlasLoadState } from "../../components/genres/GenreAtlas";
import { type SidebarDestination } from "../../components/navigation/SidebarNavigation";
import { transitionContent } from "../../contentTransition";
import {
  type GenreDetail,
  type GenreQueueMode,
  type GenreRadioSession,
  type GenreSummary,
  loadGenreDetail,
  loadGenreIndex,
  loadGenreQueue,
  saveGenreRadioSession
} from "../../genres";
import { formatCount } from "../../library";
import { type PlaybackSnapshot, usePlayback } from "../../playback";

interface GenresDomainOptions {
  activeNav: SidebarDestination;
  loadedPageRequestsRef: RefObject<Map<string, string>>;
  playback: Pick<ReturnType<typeof usePlayback>, "play"> & { state: Pick<PlaybackSnapshot, "queue" | "currentIndex">; };
  appendPlayback: ReturnType<typeof usePlayback>["append"];
  libraryReady: boolean;
}

/** Owns the genres destination's state, request guards, and actions. */
export function useGenresDomain({
  activeNav,
  loadedPageRequestsRef,
  playback,
  appendPlayback,
  libraryReady,
}: GenresDomainOptions) {
  const [genreAtlasGenres, setGenreAtlasGenres] = useState<GenreSummary[]>([]);
  const [selectedGenre, setSelectedGenre] = useState<string | null>(null);
  const [genreDetail, setGenreDetail] = useState<GenreDetail | null>(null);
  const [genreSearch, setGenreSearch] = useState("");
  const [genreIndexState, setGenreIndexState] = useState<GenreAtlasLoadState>("loading");
  const [genreDetailState, setGenreDetailState] = useState<GenreAtlasLoadState>("loading");
  const [genreIndexError, setGenreIndexError] = useState<string | null>(null);
  const [genreDetailError, setGenreDetailError] = useState<string | null>(null);
  const [genreIndexReloadToken, setGenreIndexReloadToken] = useState(0);
  const [genreDetailReloadToken, setGenreDetailReloadToken] = useState(0);
  const [genreQueueBusy, setGenreQueueBusy] = useState<GenreQueueMode | null>(null);
  const [genreQueueMessage, setGenreQueueMessage] = useState<string | null>(null);
  const [genreRadioSession, setGenreRadioSession] = useState<GenreRadioSession | null>(null);
  const genreIndexRequestRef = useRef(0);
  const genreDetailRequestRef = useRef(0);
  const genreQueueRequestRef = useRef(0);
  const genreRefillRunningRef = useRef(false);
  const selectedGenreRef = useRef(selectedGenre);

  useLayoutEffect(() => { selectedGenreRef.current = selectedGenre; }, [selectedGenre]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Genres") return;
    const pageRequestKey = String(genreIndexReloadToken);
    if (loadedPageRequestsRef.current.get("genre-index") === pageRequestKey) return;
    const requestId = ++genreIndexRequestRef.current;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setGenreIndexState("loading");
      setGenreIndexError(null);
      void loadGenreIndex()
        .then((items) => {
          if (cancelled || requestId !== genreIndexRequestRef.current) return;
          loadedPageRequestsRef.current.set("genre-index", pageRequestKey);
          setGenreAtlasGenres(items);
          setSelectedGenre((current) => current && items.some((item) => item.name === current)
            ? current
            : items[0]?.name ?? null);
          setGenreIndexState("ready");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== genreIndexRequestRef.current) return;
          setGenreIndexError(error instanceof Error ? error.message : String(error));
          setGenreIndexState("error");
        });
    }, 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, genreIndexReloadToken, loadedPageRequestsRef]);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Genres" || !selectedGenre) return;
    const pageRequestKey = JSON.stringify([selectedGenre, genreDetailReloadToken]);
    if (loadedPageRequestsRef.current.get("genre-detail") === pageRequestKey) return;
    const requestId = ++genreDetailRequestRef.current;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setGenreDetailState("loading");
      setGenreDetailError(null);
      void loadGenreDetail(selectedGenre)
        .then((detail) => {
          transitionContent(() => {
            if (cancelled || requestId !== genreDetailRequestRef.current) return;
            loadedPageRequestsRef.current.set("genre-detail", pageRequestKey);
            setGenreDetail(detail);
            setGenreDetailState("ready");
          }, "collection");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== genreDetailRequestRef.current) return;
          setGenreDetailError(error instanceof Error ? error.message : String(error));
          setGenreDetailState("error");
        });
    }, 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, selectedGenre, genreDetailReloadToken, loadedPageRequestsRef]);

  useEffect(() => {
    if (!genreRadioSession) return;
    if (playback.state.queue.length === 0 || playback.state.currentIndex === null) return;
    const remaining = playback.state.queue.length - playback.state.currentIndex - 1;
    if (remaining >= 20 || genreRefillRunningRef.current) return;
    const requestId = ++genreQueueRequestRef.current;
    genreRefillRunningRef.current = true;
    const excluded = playback.state.queue.map((track) => track.trackKey);
    void loadGenreQueue({
      genre: genreRadioSession.genre,
      mode: genreRadioSession.mode,
      limit: 100,
      excludeTrackKeys: excluded,
    })
      .then(async (tracks) => {
        if (requestId !== genreQueueRequestRef.current) return;
        if (tracks.length === 0) {
          setGenreQueueMessage(`Aurora reached the end of this ${genreRadioSession.genre} expedition.`);
          return;
        }
        const next = await appendPlayback(tracks);
        if (requestId === genreQueueRequestRef.current && next) {
          setGenreQueueMessage(`Added ${formatCount(tracks.length)} more ${genreRadioSession.genre} tracks.`);
        }
      })
      .catch((error: unknown) => {
        if (requestId === genreQueueRequestRef.current) {
          setGenreQueueMessage(`Could not refill Genre Radio: ${error instanceof Error ? error.message : String(error)}`);
        }
      })
      .finally(() => {
        if (requestId === genreQueueRequestRef.current) genreRefillRunningRef.current = false;
      });
  }, [appendPlayback, genreRadioSession, playback.state.currentIndex, playback.state.queue]);

  function endGenreQueue() {
    genreQueueRequestRef.current += 1;
    genreRefillRunningRef.current = false;
    setGenreRadioSession(null);
    saveGenreRadioSession(null);
    setGenreQueueMessage(null);
  }

  async function startGenreQueue(mode: GenreQueueMode) {
    if (!selectedGenre || genreQueueBusy) return;
    const requestedGenre = selectedGenre;
    const requestId = ++genreQueueRequestRef.current;
    genreRefillRunningRef.current = false;
    setGenreQueueBusy(mode);
    setGenreQueueMessage(null);
    try {
      const tracks = await loadGenreQueue({
        genre: requestedGenre,
        mode,
        limit: 100,
        excludeTrackKeys: [],
      });
      if (requestId !== genreQueueRequestRef.current || selectedGenreRef.current !== requestedGenre) return;
      if (tracks.length === 0) {
        setGenreQueueMessage(`No ${requestedGenre} tracks match this expedition yet.`);
        return;
      }
      const next = await playback.play(tracks, tracks[0].id);
      if (requestId !== genreQueueRequestRef.current || selectedGenreRef.current !== requestedGenre || !next) return;
      const session: GenreRadioSession = { version: 1, genre: requestedGenre, mode };
      setGenreRadioSession(session);
      saveGenreRadioSession(session);
      setGenreQueueMessage(`Loaded ${formatCount(tracks.length)} tracks. Aurora will refill with bounded batches.`);
    } catch (error) {
      if (requestId === genreQueueRequestRef.current) {
        setGenreQueueMessage(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (requestId === genreQueueRequestRef.current) setGenreQueueBusy(null);
    }
  }

  return {
    genreAtlasGenres,
    setGenreAtlasGenres,
    selectedGenre,
    setSelectedGenre,
    genreDetail,
    setGenreDetail,
    genreSearch,
    setGenreSearch,
    genreIndexState,
    genreDetailState,
    genreIndexError,
    genreDetailError,
    setGenreIndexReloadToken,
    setGenreDetailReloadToken,
    genreQueueBusy,
    genreQueueMessage,
    genreRadioSession,
    endGenreQueue,
    startGenreQueue,
  };
}
