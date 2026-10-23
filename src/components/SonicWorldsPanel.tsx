import { useContext, useEffect, useRef, useState } from "react";
import { displayTrackArtist, isTauriRuntime, type Track } from "../library";
import { sonicWorlds, type WorldBoundary, type WorldDirection, type WorldResponse } from "../sonicWorlds";
import { AddToPlaylistButton } from "./playlists/PlaylistAuthoring";
import { PlaylistAuthoringContext } from "./playlists/playlistAuthoringContext";
import "./SonicWorldsPanel.css";

interface Props { track: Track; onPlay: (track: Track, queue: Track[]) => void; onClose: () => void; onPause: () => void; playing: boolean; playbackError: string | null; }

/** Keyed by seed at the app level; playback never replaces the chosen seed. */
export function SonicWorldsPanel({ track, onPlay, onClose, onPause, playing, playbackError }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const serial = useRef(0);
  const [boundary, setBoundary] = useState<WorldBoundary>("decade");
  const [direction, setDirection] = useState<WorldDirection>("cross");
  const [response, setResponse] = useState<WorldResponse | null>(null);
  const [revealed, setRevealed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [excluded, setExcluded] = useState<string[]>([]);
  const save = useContext(PlaylistAuthoringContext);
  useEffect(() => { const requests = serial; dialog.current?.showModal(); return () => { requests.current++; }; }, []);
  function reset() { serial.current++; setResponse(null); setRevealed(false); setError(null); setBusy(false); setExcluded([]); }
  async function load(next = false) {
    const id = ++serial.current;
    const skip = next ? [...excluded, ...(response?.matches.map(m => m.track.trackKey) ?? [])].slice(-300) : [];
    setBusy(true); setResponse(null); setRevealed(false); setError(null);
    try {
      const result = await sonicWorlds({ seedKey: track.trackKey, boundary, direction, excludeKeys: skip });
      if (serial.current === id) { setResponse(result); setExcluded(skip); }
    } catch (failure) { if (serial.current === id) setError(String(failure instanceof Error ? failure.message : failure)); }
    finally { if (serial.current === id) setBusy(false); }
  }
  return <dialog ref={dialog} className="sonic-worlds" aria-labelledby="sonic-worlds-title" onCancel={onClose}>
    <header><div><p className="sonic-worlds__eyebrow">A listening challenge</p><h2 id="sonic-worlds-title">Same sound, different world</h2></div><button type="button" onClick={onClose} aria-label="Close sound challenge">Close</button></header>
    <p>Three nearby sounds, with a boundary you choose. Listen first, then reveal the connection.</p>
    <div className="sonic-worlds__seed"><div><strong>{track.title}</strong><span>{displayTrackArtist(track)} · {track.album}</span></div><button type="button" onClick={() => onPlay(track, [track])}>Play seed</button></div>
    <button type="button" disabled={!playing} onClick={onPause}>Pause playback</button>
    {playbackError && <p role="alert">{playbackError}</p>}
    <div className="sonic-worlds__controls"><label>Compare by<select value={boundary} onChange={e => { reset(); setBoundary(e.target.value as WorldBoundary); }}><option value="decade">Decade (album year)</option><option value="albumArtist">Album artist</option><option value="genre">Catalog genre</option></select></label>
      <fieldset><legend>Where should the sound take you?</legend>{([["stay", "Stay close"], ["either", "Either"], ["cross", "Cross borders"]] as const).map(([mode, label]) => <label key={mode}><input type="radio" name="world-direction" value={mode} checked={direction === mode} onChange={() => { reset(); setDirection(mode); }} />{label}</label>)}</fieldset></div>
    <p>{direction === "stay" ? "Keep the same" : direction === "cross" ? "Require a different" : "Allow any"} {boundary === "decade" ? "decade" : boundary === "albumArtist" ? "album-artist credit" : "catalog genre"}. {direction !== "either" && "Unknown values cannot satisfy this rule."}</p>
    <div className="sonic-worlds__actions"><button type="button" disabled={busy} onClick={() => void load()}>{busy ? "Finding nearby sounds…" : "Find three sounds"}</button>{response?.matches.length ? <button type="button" onClick={() => void load(true)}>Another round</button> : null}</div>
    {!isTauriRuntime() && <p role="note">Browser demonstration: sample metadata and illustrative distances; native audio analysis is used in the installed app.</p>}
    {error && <p role="alert">{error}</p>}
    {response && !response.seedReady && <p role="status">This seed needs current, available audio analysis and must not be banned. Analyze it in Music Library, then retry.</p>}
    {response?.seedReady && response.matches.length < 3 && <p role="status">{response.matches.length ? `Only ${response.matches.length} close ${response.matches.length === 1 ? "match meets" : "matches meet"}` : "No close matches meet"} this rule in the searched shortlist. Try another boundary or seed. The rule has not been relaxed.</p>}
    {response?.matches.length ? <>
      <p className="sonic-worlds__notice">Names are hidden here until reveal. The player and system media controls may show track identity during playback.</p>
      <div className="sonic-worlds__cards">{response.matches.map((match, i) => <article key={match.track.trackKey} aria-label={`Candidate ${i + 1}`}><span className="sonic-worlds__number">0{i + 1}</span>
        {revealed ? <><h3>{match.track.title}</h3><p>{displayTrackArtist(match.track)} · {match.track.album}</p><p className="sonic-worlds__connection">{response.seedValue ?? "Unknown"} → {match.value ?? "Unknown"}</p><p>Sound distance {match.distance.toFixed(3)} · sound only</p></> : <><h3>Candidate {i + 1}</h3><p>Listen for what feels familiar.</p></>}
        <div className="sonic-worlds__actions"><button type="button" aria-label={`Play candidate ${i + 1}`} onClick={() => onPlay(match.track, [match.track])}>Play</button>{revealed && <AddToPlaylistButton track={match.track} label={match.track.title} />}</div>
      </article>)}</div>
      <div className="sonic-worlds__actions">{!revealed ? <button type="button" onClick={() => setRevealed(true)}>Reveal connections</button> : save && <button type="button" onClick={() => save({ tracks: response.matches.map(m => m.track), label: "Same sound, different world" })}>Save discoveries</button>}</div>
      {revealed && <p>Decades use album year; artist comparisons use the full stored album-artist credit. Sonic closeness does not establish influence, tradition, or a shared mood. Distances are not percentages.</p>}
    </> : null}
    {response?.seedReady && <p>Searches nearby analyzed recordings, up to distance {response.maxDistance.toFixed(2)}. {response.searchLimited && "This is a bounded shortlist, not an exhaustive search of the collection."}</p>}
  </dialog>;
}
