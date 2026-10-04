import { useCallback, useLayoutEffect, useRef, useState } from "react";
import type { ArtistWorldState } from "../../components/musicbrainz/ArtistWorld";
import { transitionContent } from "../../contentTransition";
import { loadArtistDetail, type ArtistDetail } from "../../library";
import { loadArtistIntelligence, type ArtistIntelligence } from "../../musicbrainz";

export type ArtistNavigationPorts = {
  onShowArtistInspector: () => void;
  onClearCurationError: () => void;
};

/** Owns artist inspector requests; navigation and curation stay in their domains. */
export function useArtistNavigation(ports: ArtistNavigationPorts) {
  const [inspectorArtistName, setInspectorArtistName] = useState<string | null>(null);
  const [artistDetail, setArtistDetail] = useState<ArtistDetail | null>(null);
  const [artistIntelligence, setArtistIntelligence] = useState<ArtistIntelligence | null>(null);
  const [artistWorldState, setArtistWorldState] = useState<ArtistWorldState>("loading");
  const [artistWorldError, setArtistWorldError] = useState<string | null>(null);
  const artistRequestRef = useRef(0);
  const inspectorArtistNameRef = useRef(inspectorArtistName);
  const portsRef = useRef(ports);
  useLayoutEffect(() => {
    inspectorArtistNameRef.current = inspectorArtistName;
    portsRef.current = ports;
  });

  const openArtistInspector = useCallback((artistName: string) => {
    transitionContent(() => {
      const requestId = ++artistRequestRef.current;
      setInspectorArtistName(artistName);
      portsRef.current.onShowArtistInspector();
      setArtistDetail(null);
      setArtistIntelligence(null);
      setArtistWorldError(null);
      portsRef.current.onClearCurationError();
      setArtistWorldState("loading");
      void Promise.allSettled([
        loadArtistDetail(artistName).then((detail) => {
          if (requestId === artistRequestRef.current) transitionContent(() => setArtistDetail(detail), "artist-detail");
          return detail;
        }),
        loadArtistIntelligence(artistName).then((intelligence) => {
          if (requestId === artistRequestRef.current) transitionContent(() => setArtistIntelligence(intelligence), "artist-detail");
          return intelligence;
        }),
      ]).then(([catalogResult, intelligenceResult]) => {
        transitionContent(() => {
          if (requestId !== artistRequestRef.current) return;
          if (catalogResult.status === "fulfilled") setArtistDetail(catalogResult.value);
          if (intelligenceResult.status === "fulfilled") setArtistIntelligence(intelligenceResult.value);
          if (catalogResult.status === "rejected" && intelligenceResult.status === "rejected") {
            const catalogMessage = catalogResult.reason instanceof Error ? catalogResult.reason.message : String(catalogResult.reason);
            const intelligenceMessage = intelligenceResult.reason instanceof Error ? intelligenceResult.reason.message : String(intelligenceResult.reason);
            setArtistWorldError(`${catalogMessage} ${intelligenceMessage}`);
            setArtistWorldState("error");
            return;
          }
          setArtistWorldError(catalogResult.status === "rejected"
            ? "The local catalog summary is unavailable; MusicBrainz context is still shown."
            : intelligenceResult.status === "rejected"
              ? "MusicBrainz context is unavailable; the local catalog remains usable."
              : null);
          setArtistWorldState("ready");
        }, "artist-detail");
      });
    }, "artist-detail");
  }, []);

  const openArtistInspectorRef = useRef(openArtistInspector);

  return {
    inspectorArtistName, setInspectorArtistName, inspectorArtistNameRef,
    artistDetail, setArtistDetail, artistIntelligence, setArtistIntelligence,
    artistWorldState, setArtistWorldState, artistWorldError, setArtistWorldError,
    artistRequestRef, openArtistInspector, openArtistInspectorRef,
  };
}

export type ArtistNavigation = ReturnType<typeof useArtistNavigation>;
