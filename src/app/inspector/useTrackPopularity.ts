import { useEffect, useState } from "react";
import { displayTrackArtist, loadTrackPopularity, type Track } from "../../library";

/** Refresh the displayed track independently of album ranking and catalog projections. */
export function useTrackPopularity(track: Track | null): number | null {
  const trackKey = track?.trackKey;
  const artist = track ? displayTrackArtist(track) : undefined;
  const title = track?.title;
  const [result, setResult] = useState<{
    trackKey: string; artist: string; title: string; playCount: number | null;
  } | null>(null);

  useEffect(() => {
    if (!trackKey || !artist || !title) return;
    let cancelled = false;
    void loadTrackPopularity(artist, title).then(({ playCount }) => {
      if (!cancelled) setResult({ trackKey, artist, title, playCount });
    }).catch(() => {
      // Keep saved evidence if Last.fm is offline or credentials are unavailable.
    });
    return () => { cancelled = true; };
  }, [trackKey, artist, title]);

  return result && result.trackKey === trackKey && result.artist === artist && result.title === title
    ? result.playCount
    : track?.playCount ?? null;
}
