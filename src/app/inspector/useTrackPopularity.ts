import { useEffect, useState } from "react";
import { displayTrackArtist, loadTrackPopularity, type Track } from "../../library";

export interface DisplayedTrackPopularity {
  listeners: number | null;
  playCount: number | null;
}

/** Refresh the displayed track independently of album ranking and catalog projections. */
export function useTrackPopularity(track: Track | null): DisplayedTrackPopularity | null {
  const trackKey = track?.trackKey;
  const artist = track ? displayTrackArtist(track) : undefined;
  const title = track?.title;
  const [result, setResult] = useState<{
    trackKey: string; artist: string; title: string; listeners: number | null; playCount: number | null;
  } | null>(null);

  useEffect(() => {
    if (!trackKey || !artist || !title) return;
    let cancelled = false;
    void loadTrackPopularity(artist, title).then(({ listeners, playCount }) => {
      if (!cancelled) setResult({ trackKey, artist, title, listeners: listeners ?? null, playCount });
    }).catch(() => {
      // Keep saved evidence if Last.fm is offline or credentials are unavailable.
    });
    return () => { cancelled = true; };
  }, [trackKey, artist, title]);

  if (!track) return null;
  return result && result.trackKey === trackKey && result.artist === artist && result.title === title
    ? { listeners: result.listeners, playCount: result.playCount }
    : { listeners: null, playCount: track.playCount ?? null };
}
