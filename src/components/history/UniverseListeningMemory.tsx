import { ChevronRight, Clock3, Music2 } from "lucide-react";
import type { HistoryPage } from "../../history";
import { displayTrackArtist, formatCount } from "../../library";
import { Artwork } from "../Artwork";

export function UniverseListeningMemory({
  page,
  onOpenHistory,
}: {
  page: HistoryPage;
  onOpenHistory: () => void;
}) {
  const recent = page.items[0];
  const artist = recent?.track ? displayTrackArtist(recent.track) : recent?.artist;
  const album = recent?.track?.album ?? recent?.album;

  return (
    <section className="memory-strip" aria-label="Listening memory">
      <div className="memory-strip__heading">
        <Clock3 aria-hidden="true" />
        <span>
          <strong>Listening Memory</strong>
          <small>{formatCount(page.summary.plays)} registered plays across {page.devices.length || 1} {page.devices.length === 1 ? "device" : "devices"}</small>
        </span>
      </div>
      {recent ? (
        <div className="memory-strip__recent">
          {recent.track ? <Artwork track={recent.track} decorative={false} /> : <span className="memory-strip__cover-fallback" aria-hidden="true"><Music2 /></span>}
          <span className="memory-strip__recent-copy">
            <span>Last heard</span>
            <strong>{recent.title}</strong>
            <small>{artist} · {album}</small>
          </span>
        </div>
      ) : (
        <div className="memory-strip__recent memory-strip__recent--empty">
          <span className="memory-strip__recent-copy">
            <span>Ready to remember</span>
            <strong>Play something you love</strong>
            <small>It counts after {page.playThresholdSeconds} seconds</small>
          </span>
        </div>
      )}
      <button type="button" onClick={onOpenHistory}>Open History <ChevronRight aria-hidden="true" /></button>
    </section>
  );
}
