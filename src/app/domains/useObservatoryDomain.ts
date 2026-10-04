import { useEffect, useRef, useState, type Dispatch, type RefObject, type SetStateAction } from "react";
import { type ObservatoryLoadState } from "../../components/curation/Observatory";
import { type ArtistWorldState } from "../../components/musicbrainz/ArtistWorld";
import { type SidebarDestination } from "../../components/navigation/SidebarNavigation";
import { transitionContent } from "../../contentTransition";
import { formatCount, loadArtistDetail, type ArtistDetail } from "../../library";
import {
  exportMusicBrainzCuration,
  loadArtistReviewPage,
  undoMusicBrainzCuration,
  updateArtistIdentityDecision,
  updateReleaseGroupDecision,
  type ArtistDecisionRequest,
  type ArtistIntelligence,
  type ArtistReviewFilter,
  type ArtistReviewItem,
  type ReleaseDecisionRequest
} from "../../musicbrainz";
import { type InspectorView } from "../../viewPreferences";

interface ObservatoryDomainOptions {
  setInspectorView: Dispatch<SetStateAction<InspectorView>>;
  inspectorArtistName: string | null;
  setInspectorArtistName: Dispatch<SetStateAction<string | null>>;
  setArtistDetail: Dispatch<SetStateAction<ArtistDetail | null>>;
  setArtistIntelligence: Dispatch<SetStateAction<ArtistIntelligence | null>>;
  setArtistWorldState: Dispatch<SetStateAction<ArtistWorldState>>;
  activeNav: SidebarDestination;
  artistRequestRef: RefObject<number>;
  loadedPageRequestsRef: RefObject<Map<string, string>>;
  libraryReady: boolean;
}

/** Owns the observatory destination's state, request guards, and actions. */
export function useObservatoryDomain({
  setInspectorView,
  inspectorArtistName,
  setInspectorArtistName,
  setArtistDetail,
  setArtistIntelligence,
  setArtistWorldState,
  activeNav,
  artistRequestRef,
  loadedPageRequestsRef,
  libraryReady,
}: ObservatoryDomainOptions) {
  const [curationError, setCurationError] = useState<string | null>(null);
  const [curationActionBusy, setCurationActionBusy] = useState<string | null>(null);
  const [curationMessage, setCurationMessage] = useState<string | null>(null);
  const [reviewItems, setReviewItems] = useState<ArtistReviewItem[]>([]);
  const [reviewCursor, setReviewCursor] = useState<string | null>(null);
  const [reviewFilter, setReviewFilter] = useState<ArtistReviewFilter>("needsReview");
  const [reviewSearch, setReviewSearch] = useState("");
  const [reviewLoadState, setReviewLoadState] = useState<ObservatoryLoadState>("loading");
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [reviewLoadingMore, setReviewLoadingMore] = useState(false);
  const [reviewReloadToken, setReviewReloadToken] = useState(0);
  const reviewRequestRef = useRef(0);

  useEffect(() => {
    if (!libraryReady || activeNav !== "Observatory") return;
    const pageRequestKey = JSON.stringify([reviewFilter, reviewSearch, reviewReloadToken]);
    if (loadedPageRequestsRef.current.get("observatory") === pageRequestKey) return;
    const requestId = ++reviewRequestRef.current;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setReviewLoadState("loading");
      setReviewError(null);
      setReviewCursor(null);
      void loadArtistReviewPage({ pageSize: 50, filter: reviewFilter, search: reviewSearch.trim() || undefined })
        .then((page) => {
          transitionContent(() => {
            if (cancelled || requestId !== reviewRequestRef.current) return;
            loadedPageRequestsRef.current.set("observatory", pageRequestKey);
            setReviewItems(page.items);
            setReviewCursor(page.nextCursor);
            setReviewLoadState("ready");
          }, "page");
        })
        .catch((error: unknown) => {
          if (cancelled || requestId !== reviewRequestRef.current) return;
          setReviewError(error instanceof Error ? error.message : String(error));
          setReviewLoadState("error");
        });
    }, reviewSearch.trim() ? 160 : 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activeNav, libraryReady, reviewFilter, reviewSearch, reviewReloadToken, loadedPageRequestsRef]);

  async function applyArtistDecision(request: ArtistDecisionRequest) {
    if (curationActionBusy) return;
    const requestId = ++artistRequestRef.current;
    setCurationActionBusy("artist");
    setCurationError(null);
    setCurationMessage(null);
    try {
      const intelligence = await updateArtistIdentityDecision(request);
      if (requestId !== artistRequestRef.current || inspectorArtistName !== request.artist) return;
      setArtistIntelligence(intelligence);
      setArtistWorldState("ready");
      setCurationMessage(request.action === "clear"
        ? `Cleared Aurora's identity override for ${request.artist}.`
        : request.action === "ignore"
          ? `Ignored ${request.artist} in Aurora.`
          : `Confirmed ${request.artist} as ${intelligence.identity?.canonicalName ?? request.artist}.`);
      setReviewReloadToken((value) => value + 1);
    } catch (error) {
      if (requestId === artistRequestRef.current) {
        setCurationError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      setCurationActionBusy((current) => current === "artist" ? null : current);
    }
  }

  async function applyReleaseDecision(request: ReleaseDecisionRequest) {
    if (curationActionBusy) return;
    const requestId = ++artistRequestRef.current;
    setCurationActionBusy(`release:${request.releaseMbid}`);
    setCurationError(null);
    setCurationMessage(null);
    try {
      const intelligence = await updateReleaseGroupDecision(request);
      if (requestId !== artistRequestRef.current || inspectorArtistName !== request.artist) return;
      setArtistIntelligence(intelligence);
      setArtistWorldState("ready");
      setCurationMessage(request.action === "link"
        ? "Linked the MusicBrainz release group to the selected local album."
        : request.action === "notInScope"
          ? "Marked the release group as not in scope."
          : request.action === "ignore"
            ? "Ignored the release group."
            : "Cleared Aurora's release override.");
      setReviewReloadToken((value) => value + 1);
    } catch (error) {
      if (requestId === artistRequestRef.current) {
        setCurationError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      setCurationActionBusy((current) => current === `release:${request.releaseMbid}` ? null : current);
    }
  }

  async function loadMoreReviewItems() {
    if (!reviewCursor || reviewLoadingMore) return;
    const requestId = ++reviewRequestRef.current;
    setReviewLoadingMore(true);
    try {
      const page = await loadArtistReviewPage({
        pageSize: 50,
        cursor: reviewCursor,
        filter: reviewFilter,
        search: reviewSearch.trim() || undefined,
      });
      if (requestId !== reviewRequestRef.current) return;
      setReviewItems((current) => {
        const existing = new Set(current.map((item) => item.artistKey));
        return [...current, ...page.items.filter((item) => !existing.has(item.artistKey))];
      });
      setReviewCursor(page.nextCursor);
    } catch (error) {
      if (requestId === reviewRequestRef.current) {
        setReviewError(error instanceof Error ? error.message : String(error));
        setReviewLoadState("error");
      }
    } finally {
      if (requestId === reviewRequestRef.current) setReviewLoadingMore(false);
    }
  }

  async function undoCuration() {
    if (curationActionBusy) return;
    setCurationActionBusy("undo");
    setCurationMessage(null);
    try {
      const intelligence = await undoMusicBrainzCuration();
      if (!intelligence) {
        setCurationMessage("There is no Aurora curation decision to undo.");
        return;
      }
      setInspectorArtistName(intelligence.artist);
      setInspectorView("artist");
      setArtistIntelligence(intelligence);
      setArtistWorldState("ready");
      setCurationError(null);
      setCurationMessage(`Undid the latest decision for ${intelligence.artist}.`);
      void loadArtistDetail(intelligence.artist).then(setArtistDetail).catch(() => setArtistDetail(null));
      setReviewReloadToken((value) => value + 1);
    } catch (error) {
      setCurationMessage(`Could not undo the latest decision: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setCurationActionBusy(null);
    }
  }

  async function exportCuration() {
    if (curationActionBusy) return;
    setCurationActionBusy("export");
    setCurationMessage(null);
    try {
      const result = await exportMusicBrainzCuration();
      setCurationMessage(`Exported ${formatCount(result.artistDecisions)} artist and ${formatCount(result.releaseDecisions)} release decisions to ${result.path}`);
    } catch (error) {
      setCurationMessage(`Could not export the overlay snapshot: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setCurationActionBusy(null);
    }
  }

  return {
    curationError,
    setCurationError,
    curationActionBusy,
    curationMessage,
    reviewItems,
    reviewCursor,
    reviewFilter,
    setReviewFilter,
    reviewSearch,
    setReviewSearch,
    reviewLoadState,
    reviewError,
    reviewLoadingMore,
    setReviewLoadingMore,
    setReviewReloadToken,
    applyArtistDecision,
    applyReleaseDecision,
    loadMoreReviewItems,
    undoCuration,
    exportCuration,
  };
}
