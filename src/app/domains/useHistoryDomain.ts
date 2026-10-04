import { useEffect, useRef, useState, type RefObject } from "react";
import { type HistoryDateRange, type HistoryLoadState } from "../../components/history/ListeningHistory";
import { type SidebarDestination } from "../../components/navigation/SidebarNavigation";
import { loadHistoryPage, saveHistoryPlayThreshold, type HistoryOutcomeFilter, type HistoryPage } from "../../history";
import { type Track } from "../../library";
import { usePlayback } from "../../playback";

interface HistoryDomainOptions {
  activeNav: SidebarDestination;
  loadedPageRequestsRef: RefObject<Map<string, string>>;
  playback: Pick<ReturnType<typeof usePlayback>, "play">;
  libraryReady: boolean;
  endGenreQueue: () => void;
  selectTrack: (track: Track) => void;
}

/** Owns the history destination's state, request guards, and actions. */
export function useHistoryDomain({
  activeNav,
  loadedPageRequestsRef,
  playback,
  libraryReady,
  endGenreQueue,
  selectTrack,
}: HistoryDomainOptions) {
  const [historyPage, setHistoryPage] = useState<HistoryPage | null>(null);
  const [historyLoadState, setHistoryLoadState] = useState<HistoryLoadState>("loading");
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [historySearch, setHistorySearch] = useState("");
  const [historyOutcome, setHistoryOutcome] = useState<HistoryOutcomeFilter>("all");
  const [historyDeviceId, setHistoryDeviceId] = useState<string | null>(null);
  const [historyDateRange, setHistoryDateRange] = useState<HistoryDateRange>("all");
  const [historyLoadingMore, setHistoryLoadingMore] = useState(false);
  const [historySavingThreshold, setHistorySavingThreshold] = useState(false);
  const [historyThresholdMessage, setHistoryThresholdMessage] = useState<string | null>(null);
  const [historyReloadToken, setHistoryReloadToken] = useState(0);
  const historyRequestRef = useRef(0);
  const [universeHistoryPage, setUniverseHistoryPage] = useState<HistoryPage | null>(null);

  useEffect(() => {
    if (!libraryReady || (activeNav !== "History" && activeNav !== "Universe")) return;
    const pageRequestKey = JSON.stringify(activeNav === "History" ? [historySearch, historyOutcome, historyDeviceId, historyDateRange, historyReloadToken] : [historyReloadToken]);
    if (loadedPageRequestsRef.current.get(activeNav) === pageRequestKey) return;
    const requestId = ++historyRequestRef.current;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (activeNav === "History") setHistoryLoadState("loading");
      setHistoryError(null);
      const startedAfterMs = activeNav === "History" && historyDateRange !== "all"
        ? Date.now() - Number(historyDateRange) * 86_400_000
        : undefined;
      void loadHistoryPage({
        pageSize: activeNav === "History" ? 50 : 5,
        search: activeNav === "History" ? historySearch.trim() || undefined : undefined,
        outcome: activeNav === "History" ? historyOutcome : "all",
        deviceId: activeNav === "History" ? historyDeviceId ?? undefined : undefined,
        startedAfterMs,
      })
        .then((page) => {
          if (cancelled || requestId !== historyRequestRef.current) return;
          loadedPageRequestsRef.current.set(activeNav, pageRequestKey);
          if (activeNav === "Universe") setUniverseHistoryPage(page);
          else setHistoryPage(page);
          setHistoryLoadState("ready");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== historyRequestRef.current) return;
          setHistoryError(error instanceof Error ? error.message : String(error));
          setHistoryLoadState("error");
        });
    }, activeNav === "History" && historySearch.trim() ? 160 : 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [
    activeNav,
    libraryReady,
    historySearch,
    historyOutcome,
    historyDeviceId,
    historyDateRange,
    historyReloadToken,
    loadedPageRequestsRef,
  ]);

  useEffect(() => {
    if (activeNav !== "History") return;
    const interval = window.setInterval(() => {
      setHistoryReloadToken((value) => value + 1);
    }, 15_000);
    return () => window.clearInterval(interval);
  }, [activeNav]);

  async function loadMoreHistory() {
    if (!historyPage?.nextCursor || historyLoadingMore) return;
    const requestId = ++historyRequestRef.current;
    setHistoryLoadingMore(true);
    try {
      const startedAfterMs = historyDateRange === "all"
        ? undefined
        : Date.now() - Number(historyDateRange) * 86_400_000;
      const next = await loadHistoryPage({
        pageSize: 50,
        cursor: historyPage.nextCursor,
        search: historySearch.trim() || undefined,
        outcome: historyOutcome,
        deviceId: historyDeviceId ?? undefined,
        startedAfterMs,
      });
      if (requestId !== historyRequestRef.current) return;
      setHistoryPage((current) => current ? {
        ...next,
        items: [...current.items, ...next.items],
      } : next);
    } catch (error) {
      if (requestId === historyRequestRef.current) {
        setHistoryError(error instanceof Error ? error.message : String(error));
        setHistoryLoadState("error");
      }
    } finally {
      if (requestId === historyRequestRef.current) setHistoryLoadingMore(false);
    }
  }

  async function savePlayedThreshold(value: number) {
    setHistorySavingThreshold(true);
    setHistoryThresholdMessage(null);
    try {
      const saved = await saveHistoryPlayThreshold(value);
      setHistoryPage((current) => current ? { ...current, playThresholdSeconds: saved } : current);
      setHistoryThresholdMessage(`A play now registers after ${saved} ${saved === 1 ? "second" : "seconds"}.`);
    } catch (error) {
      setHistoryThresholdMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setHistorySavingThreshold(false);
    }
  }

  function playHistoryTrack(track: Track) {
    endGenreQueue();
    selectTrack(track);
    void playback.play([track], track.id);
  }

  return {
    historyPage,
    historyLoadState,
    historyError,
    historySearch,
    setHistorySearch,
    historyOutcome,
    setHistoryOutcome,
    historyDeviceId,
    setHistoryDeviceId,
    historyDateRange,
    setHistoryDateRange,
    historyLoadingMore,
    setHistoryLoadingMore,
    historySavingThreshold,
    historyThresholdMessage,
    setHistoryReloadToken,
    universeHistoryPage,
    loadMoreHistory,
    savePlayedThreshold,
    playHistoryTrack,
  };
}
