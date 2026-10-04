import { useCallback, useRef } from "react";
import { advanceCatalogTrackProjectionTokens } from "../../tags";

/** One projection clock shared by catalog refreshes and per-track tag edits. */
export function useCatalogProjection() {
  const latestTagProjectionTokenRef = useRef(0);
  const latestTrackProjectionTokensRef = useRef<ReadonlyMap<string, number>>(new Map());

  const acceptTrackProjectionKeys = useCallback((
    trackKeys: readonly string[],
    projectionToken: number | null | undefined,
  ) => {
    const decision = advanceCatalogTrackProjectionTokens(
      latestTagProjectionTokenRef.current,
      latestTrackProjectionTokensRef.current,
      projectionToken,
      trackKeys,
    );
    latestTagProjectionTokenRef.current = decision.latestToken;
    latestTrackProjectionTokensRef.current = decision.latestTrackTokens;
    return decision;
  }, []);

  return { latestTagProjectionTokenRef, latestTrackProjectionTokensRef, acceptTrackProjectionKeys };
}
