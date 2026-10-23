import { useContext, useEffect, useRef, useState } from "react";
import { displayTrackArtist, isTauriRuntime, loadAlbumDetail, type Track } from "../library";
import { sonicWorlds, type WorldResponse } from "../sonicWorlds";
import { blindTastingPlayback, type PlaybackSnapshot } from "../playback";
import { PlaylistAuthoringContext } from "./playlists/playlistAuthoringContext";
import { Artwork } from "./Artwork";
import "./BlindTasting.css";

type Match = WorldResponse["matches"][number];
interface Props {
  seed: Track; playback: PlaybackSnapshot;
  onSnapshot: (snapshot: PlaybackSnapshot) => void;
  onPause: () => void; onSeek: (seconds: number) => void;
  onAlbum: (tracks: Track[]) => Promise<unknown>; onClose: () => void;
}

export function BlindTasting({ seed, playback, onSnapshot, onPause, onSeek, onAlbum, onClose }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const generation = useRef(0);
  const [matches, setMatches] = useState<Match[]>([]);
  const [excluded, setExcluded] = useState<string[]>([]);
  const [heard, setHeard] = useState<string[]>([]);
  const [choice, setChoice] = useState<string | null>(null);
  const [revealed, setRevealed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const save = useContext(PlaylistAuthoringContext);
  useEffect(() => { dialog.current?.showModal(); const request = generation; return () => { request.current++; }; }, []);
  async function stop() { onSnapshot(await blindTastingPlayback()); }
  async function close() { if (busy) return; setBusy(true); try { await stop(); onClose(); } catch { setError("Playback could not stop. Try closing again."); setBusy(false); } }
  async function round() {
    const id = ++generation.current;
    setBusy(true); setError(null); setRevealed(false); setChoice(null); setHeard([]);
    const skip = [...excluded, ...matches.map(m => m.track.trackKey)].slice(-300);
    setMatches([]);
    try {
      await stop();
      const result = await sonicWorlds({ seedKey: seed.trackKey, boundary: "albumArtist", direction: "either", excludeKeys: skip });
      if (id !== generation.current) return;
      if (!result.seedReady) throw new Error("Analyze this seed in Music Library, then try again.");
      if (result.matches.length < 2) throw new Error("There are not two fresh, distinct recordings in this shortlist. Try another seed.");
      const pair = result.matches.slice(0, 2);
      if (crypto.getRandomValues(new Uint8Array(1))[0] & 1) pair.reverse();
      setMatches(pair); setExcluded(skip);
    } catch (e) { if (id === generation.current) setError(e instanceof Error ? e.message : "The round could not be prepared."); }
    finally { if (id === generation.current) setBusy(false); }
  }
  async function play(track: Track, label: string) {
    setBusy(true); setError(null);
    try { onSnapshot(await blindTastingPlayback(track, label)); setHeard(h => [...new Set([...h, track.trackKey])]); }
    catch { setError("This sound could not play with track normalization. Check its file and analysis, then retry."); }
    finally { setBusy(false); }
  }
  async function album(track: Track) {
    if (!track.albumId) return;
    setBusy(true); setError(null);
    try { const detail = await loadAlbumDetail(track.albumId); await stop(); await onAlbum(detail.tracks); onClose(); }
    catch { setError("The album could not be played. Try again."); setBusy(false); }
  }
  const active = playback.blindTastingLabel;
  return <dialog ref={dialog} className="blind-tasting" aria-labelledby="tasting-title" onCancel={e => { e.preventDefault(); void close(); }}>
    <header><div><p className="tasting-kicker">Follow your ears</p><h2 id="tasting-title">Blind tasting</h2></div><button disabled={busy} onClick={() => void close()}>Close</button></header>
    <p>A familiar sound. Two hidden discoveries. Which would you keep playing?</p>
    <section className="tasting-seed"><div><small>Your seed</small><h3>{seed.title}</h3><p>{displayTrackArtist(seed)} · {seed.album}</p></div><button disabled={busy || !matches.length} onClick={() => void play(seed, "Seed")}>Play seed</button></section>
    {!isTauriRuntime() && <p role="note">Browser demonstration: sample candidates, no native audio.</p>}
    <p className="tasting-note">Volumes are matched using ReplayGain or Aurora’s estimate. Your audio settings stay unchanged.</p>
    {active && <section className="tasting-transport" aria-label="Tasting playback"><strong>{active}</strong><button disabled={busy} onClick={onPause}>{playback.status === "playing" ? "Pause" : "Resume"}</button><label>Seek<input type="range" min="0" max={playback.currentTrack?.durationSeconds ?? 0} value={Math.min(playback.positionSeconds, playback.currentTrack?.durationSeconds ?? 0)} onChange={e => onSeek(Number(e.target.value))} /></label></section>}
    <div className="tasting-pair">{matches.map((m, i) => <article key={m.track.trackKey} aria-label={`Candidate ${i ? "B" : "A"}`}>
      <span className="tasting-letter">{i ? "B" : "A"}</span>
      {revealed ? <><Artwork track={m.track} size="player" /><h3>{m.track.title}</h3><p>{displayTrackArtist(m.track)} · {m.track.album}</p><p>Whole-track sound distance: {m.distance.toFixed(3)}. A close acoustic neighbor, not a predicted preference.</p>{choice === m.track.trackKey && <strong>Your pick</strong>}</> : <><h3>A sound to discover</h3><p>No names. No covers. Just listen.</p></>}
      <button disabled={busy} onClick={() => void play(m.track, `Candidate ${i ? "B" : "A"}`)}>Play {i ? "B" : "A"}</button>
      {!revealed ? <button disabled={busy || matches.some(candidate => !heard.includes(candidate.track.trackKey))} onClick={() => { setChoice(m.track.trackKey); setRevealed(true); if (playback.currentTrack?.trackKey !== m.track.trackKey) void play(m.track, `Candidate ${i ? "B" : "A"}`); }}>Keep {i ? "B" : "A"} playing · reveal</button> : <><button disabled={!save || busy} onClick={() => save?.({ tracks: [m.track], label: m.track.title })}>Save</button><button disabled={!m.track.albumId || busy} onClick={() => void album(m.track)}>Hear the album</button></>}
    </article>)}</div>
    <footer><button disabled={busy} onClick={() => void round()}>{busy ? "Working…" : matches.length ? "Another round" : "Prepare a round"}</button>{matches.length > 0 && !revealed && <button disabled={busy} onClick={() => setRevealed(true)}>Reveal without choosing</button>}</footer>
    <p className="tasting-note">Full songs, with your own seeking. Your choice changes no rating or Ban. System media controls use anonymous labels; external devices and other apps may still identify the recording.</p>
    {error && <p role="alert">{error}</p>}
  </dialog>;
}
