import {
  ListMusic,
  Pause,
  Play,
  Repeat2,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume2,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { formatDuration, type Track } from "../library";
import type { PlaybackSnapshot, RepeatMode } from "../playback";
import type { LoveState } from "../tags";
import { loadTrackWaveform, type TrackWaveform } from "../waveform";
import { Artwork } from "./Artwork";
import { InlineLoveControl, InlineRatingControl } from "./InlineTagControls";
import { WaveformTimeline } from "./WaveformTimeline";

function nextRepeatMode(current: RepeatMode): RepeatMode {
  if (current === "off") return "all";
  if (current === "all") return "one";
  return "off";
}

function technicalSummary(waveform: TrackWaveform | null, failed: boolean): string {
  if (failed) return "MP3 waveform unavailable";
  if (!waveform) return "Reading MP3 waveform…";
  const sampleRate = waveform.sampleRate
    ? `${Number((waveform.sampleRate / 1_000).toFixed(1))} kHz`
    : null;
  const channels = waveform.channels === 1 ? "Mono" : waveform.channels === 2 ? "Stereo" : waveform.channels ? `${waveform.channels} channels` : null;
  return ["MP3", sampleRate, channels].filter(Boolean).join(" · ");
}

export function PlayerBar({
  playback,
  isWorking,
  tagBusy,
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
  onRatingChange,
  onLoveChange,
  onToggleQueue,
}: {
  playback: PlaybackSnapshot;
  isWorking: boolean;
  tagBusy: boolean;
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
  onRatingChange: (track: Track, rating: number | null) => void;
  onLoveChange: (track: Track, loveState: LoveState) => void;
  onToggleQueue: () => void;
}) {
  const [seekDraft, setSeekDraft] = useState<number | null>(null);
  const [volumeDraft, setVolumeDraft] = useState<number | null>(null);
  const [showRemaining, setShowRemaining] = useState(false);
  const [waveformResult, setWaveformResult] = useState<{
    trackKey: string;
    waveform: TrackWaveform | null;
    failed: boolean;
  } | null>(null);
  const track = playback.currentTrack;
  const trackId = track?.id ?? null;
  const trackKey = track?.trackKey ?? null;
  const waveform = waveformResult?.trackKey === trackKey ? waveformResult.waveform : null;
  const waveformFailed = waveformResult?.trackKey === trackKey && waveformResult.failed;
  const duration = Math.max(track?.durationSeconds ?? 0, 0);
  const position = Math.min(seekDraft ?? playback.positionSeconds, duration);
  const volume = volumeDraft ?? playback.volume;

  useEffect(() => {
    let cancelled = false;
    if (!trackId || !trackKey) return () => { cancelled = true; };
    void loadTrackWaveform({ id: trackId, trackKey })
      .then((next) => {
        if (!cancelled) setWaveformResult({ trackKey, waveform: next, failed: false });
      })
      .catch(() => {
        if (!cancelled) setWaveformResult({ trackKey, waveform: null, failed: true });
      });
    return () => { cancelled = true; };
  }, [trackId, trackKey]);

  function commitSeek() {
    if (seekDraft !== null) onSeek(seekDraft);
    setSeekDraft(null);
  }

  function commitVolume() {
    if (volumeDraft !== null) onVolume(volumeDraft);
    setVolumeDraft(null);
  }

  const endTime = showRemaining
    ? `−${formatDuration(Math.max(duration - position, 0))}`
    : formatDuration(track?.durationSeconds ?? null);

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
            <Artwork track={track} size="player" />
            <div className="now-playing__copy">
              <div className="now-playing__title">
                <strong>{track.title}</strong>
                <InlineLoveControl
                  title={track.title}
                  loveState={track.loveState}
                  busy={tagBusy}
                  onLoveChange={(loveState) => onLoveChange(track, loveState)}
                />
              </div>
              <span>{track.artist} · {track.album}</span>
              <div className="now-playing__details">
                <small>{technicalSummary(waveform, waveformFailed)}</small>
                <InlineRatingControl
                  title={track.title}
                  rating={track.rating}
                  busy={tagBusy}
                  allowClear
                  onRatingChange={(rating) => onRatingChange(track, rating)}
                />
              </div>
            </div>
          </div>
        ) : (
          <div className="now-playing now-playing--empty"><ListMusic aria-hidden="true" /><span>Double-click a song to begin listening</span></div>
        )}

        <div className="player__center">
          <div className="transport" aria-label="Playback controls">
            <button type="button" aria-label={playback.shuffle ? "Disable shuffle" : "Enable shuffle"} aria-pressed={playback.shuffle} className={playback.shuffle ? "is-active" : undefined} disabled={!playback.queue.length || isWorking} onClick={() => onShuffle(!playback.shuffle)}><Shuffle aria-hidden="true" /></button>
            <button type="button" aria-label="Previous track" disabled={!track || isWorking} onClick={onPrevious}><SkipBack aria-hidden="true" /></button>
            <button type="button" className="transport__play" aria-label={playback.status === "playing" ? "Pause" : "Play"} disabled={!track || isWorking} onClick={onToggle}>
              {playback.status === "playing" ? <Pause aria-hidden="true" /> : <Play aria-hidden="true" />}
            </button>
            <button type="button" aria-label="Next track" disabled={!track || isWorking} onClick={onNext}><SkipForward aria-hidden="true" /></button>
            <button type="button" aria-label={`Repeat ${playback.repeatMode}`} aria-pressed={playback.repeatMode !== "off"} className={playback.repeatMode !== "off" ? "is-active" : undefined} disabled={!playback.queue.length || isWorking} onClick={() => onRepeat(nextRepeatMode(playback.repeatMode))}>
              <Repeat2 aria-hidden="true" /><small>{playback.repeatMode === "one" ? "1" : ""}</small>
            </button>
          </div>
          <div className="timeline">
            <span>{formatDuration(position)}</span>
            <WaveformTimeline
              waveform={waveform}
              position={position}
              duration={duration}
              disabled={!track}
              onChange={setSeekDraft}
              onCommit={commitSeek}
            />
            <button
              type="button"
              className="timeline__end"
              aria-label={showRemaining ? "Show total track length" : "Show remaining track time"}
              aria-pressed={showRemaining}
              disabled={!track}
              onClick={() => setShowRemaining((current) => !current)}
            >
              {endTime}
            </button>
          </div>
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
