import {
  ChevronLeft,
  ChevronRight,
  Heart,
  ListMusic,
  Pause,
  Play,
  Repeat2,
  Shuffle,
  Volume2,
  X,
} from "lucide-react";
import { useState } from "react";
import { formatDuration } from "../library";
import type { PlaybackSnapshot, RepeatMode } from "../playback";
import { Artwork } from "./Artwork";

function nextRepeatMode(current: RepeatMode): RepeatMode {
  if (current === "off") return "all";
  if (current === "all") return "one";
  return "off";
}

export function PlayerBar({
  playback,
  isWorking,
  error,
  queueOpen,
  onDismissError,
  onToggle,
  onPrevious,
  onNext,
  onSeek,
  onVolume,
  onShuffle,
  onRepeat,
  onToggleQueue,
}: {
  playback: PlaybackSnapshot;
  isWorking: boolean;
  error: string | null;
  queueOpen: boolean;
  onDismissError: () => void;
  onToggle: () => void;
  onPrevious: () => void;
  onNext: () => void;
  onSeek: (position: number) => void;
  onVolume: (volume: number) => void;
  onShuffle: (enabled: boolean) => void;
  onRepeat: (mode: RepeatMode) => void;
  onToggleQueue: () => void;
}) {
  const [seekDraft, setSeekDraft] = useState<number | null>(null);
  const [volumeDraft, setVolumeDraft] = useState<number | null>(null);
  const track = playback.currentTrack;
  const duration = Math.max(track?.durationSeconds ?? 0, 0);
  const position = Math.min(seekDraft ?? playback.positionSeconds, duration);
  const volume = volumeDraft ?? playback.volume;

  function commitSeek() {
    if (seekDraft !== null) onSeek(seekDraft);
    setSeekDraft(null);
  }

  function commitVolume() {
    if (volumeDraft !== null) onVolume(volumeDraft);
    setVolumeDraft(null);
  }

  return (
    <>
      {error && (
        <div className="playback-error" role="alert">
          <span>{error}</span>
          <button type="button" aria-label="Dismiss playback error" onClick={onDismissError}><X aria-hidden="true" /></button>
        </div>
      )}
      <footer className="player">
        {track ? (
          <div className="now-playing">
            <Artwork track={track} />
            <div><strong>{track.title}</strong><span>{track.artist} · {track.album}</span></div>
            {track.loved && <Heart className="loved" aria-label="Loved" />}
          </div>
        ) : (
          <div className="now-playing now-playing--empty"><ListMusic aria-hidden="true" /><span>Double-click a song to begin listening</span></div>
        )}
        <div className="transport" aria-label="Playback controls">
          <button type="button" aria-label={playback.shuffle ? "Disable shuffle" : "Enable shuffle"} aria-pressed={playback.shuffle} className={playback.shuffle ? "is-active" : undefined} disabled={!playback.queue.length || isWorking} onClick={() => onShuffle(!playback.shuffle)}><Shuffle aria-hidden="true" /></button>
          <button type="button" aria-label="Previous track" disabled={!track || isWorking} onClick={onPrevious}><ChevronLeft aria-hidden="true" /></button>
          <button type="button" className="transport__play" aria-label={playback.status === "playing" ? "Pause" : "Play"} disabled={!track || isWorking} onClick={onToggle}>
            {playback.status === "playing" ? <Pause aria-hidden="true" /> : <Play aria-hidden="true" />}
          </button>
          <button type="button" aria-label="Next track" disabled={!track || isWorking} onClick={onNext}><ChevronRight aria-hidden="true" /></button>
          <button type="button" aria-label={`Repeat ${playback.repeatMode}`} aria-pressed={playback.repeatMode !== "off"} className={playback.repeatMode !== "off" ? "is-active" : undefined} disabled={!playback.queue.length || isWorking} onClick={() => onRepeat(nextRepeatMode(playback.repeatMode))}>
            <Repeat2 aria-hidden="true" /><small>{playback.repeatMode === "one" ? "1" : ""}</small>
          </button>
        </div>
        <div className="timeline">
          <span>{formatDuration(position)}</span>
          <input
            type="range"
            aria-label="Playback position"
            min={0}
            max={Math.max(duration, 1)}
            step={1}
            value={position}
            disabled={!track}
            onChange={(event) => setSeekDraft(Number(event.target.value))}
            onPointerUp={commitSeek}
            onKeyUp={commitSeek}
          />
          <span>{formatDuration(track?.durationSeconds ?? null)}</span>
        </div>
        <div className="volume">
          <Volume2 aria-hidden="true" />
          <input
            type="range"
            aria-label="Volume"
            min={0}
            max={1}
            step={0.01}
            value={volume}
            onChange={(event) => setVolumeDraft(Number(event.target.value))}
            onPointerUp={commitVolume}
            onKeyUp={commitVolume}
          />
          <small>{Math.round(volume * 100)}</small>
          <button type="button" className={queueOpen ? "is-active" : undefined} aria-label={queueOpen ? "Close queue" : "Open queue"} aria-expanded={queueOpen} onClick={onToggleQueue}>
            <ListMusic aria-hidden="true" /><span>{playback.queue.length}</span>
          </button>
        </div>
      </footer>
    </>
  );
}
