import { ArrowDown, ArrowUp, ListMusic, Trash2, X } from "lucide-react";
import { Artwork } from "./Artwork";
import type { PlaybackSnapshot } from "../playback";

export function QueuePanel({
  playback,
  onClose,
  onPlay,
  onMove,
  onRemove,
  onClear,
}: {
  playback: PlaybackSnapshot;
  onClose: () => void;
  onPlay: (trackId: string) => void;
  onMove: (from: number, to: number) => void;
  onRemove: (index: number) => void;
  onClear: () => void;
}) {
  return (
    <aside className="queue-panel" aria-labelledby="queue-title">
      <div className="queue-panel__header">
        <div>
          <p className="eyebrow">Listening order</p>
          <h2 id="queue-title">Queue <span>{playback.queue.length}</span></h2>
        </div>
        <button type="button" aria-label="Close queue" onClick={onClose}><X aria-hidden="true" /></button>
      </div>
      {playback.queue.length ? (
        <ol className="queue-list">
          {playback.queue.map((track, index) => {
            const isCurrent = playback.currentIndex === index;
            return (
              <li className={isCurrent ? "is-current" : undefined} key={`${track.id}-${index}`}>
                <button
                  type="button"
                  className="queue-track"
                  onClick={() => onPlay(track.id)}
                  aria-current={isCurrent ? "true" : undefined}
                >
                  <span className="queue-position">{isCurrent ? <ListMusic aria-hidden="true" /> : index + 1}</span>
                  <Artwork track={track} />
                  <span className="queue-copy"><strong>{track.title}</strong><small>{track.artist} · {track.album}</small></span>
                </button>
                <span className="queue-actions">
                  <button type="button" aria-label={`Move ${track.title} up`} disabled={index === 0} onClick={() => onMove(index, index - 1)}><ArrowUp aria-hidden="true" /></button>
                  <button type="button" aria-label={`Move ${track.title} down`} disabled={index === playback.queue.length - 1} onClick={() => onMove(index, index + 1)}><ArrowDown aria-hidden="true" /></button>
                  <button type="button" aria-label={`Remove ${track.title} from queue`} onClick={() => onRemove(index)}><X aria-hidden="true" /></button>
                </span>
              </li>
            );
          })}
        </ol>
      ) : (
        <div className="queue-empty"><ListMusic aria-hidden="true" /><h3>Your queue is empty</h3><p>Double-click a song to begin listening.</p></div>
      )}
      <div className="queue-panel__footer">
        <span>Queue changes are saved automatically.</span>
        <button type="button" disabled={!playback.queue.length} onClick={onClear}><Trash2 aria-hidden="true" /> Clear</button>
      </div>
    </aside>
  );
}
