import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { analyzeSonicSeed, sonicMatches, type SonicResponse } from "../sonic";
import { displayTrackArtist, type Track } from "../library";
import "./SonicPanel.css";
interface Props { track: Track; onPlay: (track: Track, queue: Track[]) => void; onRadio: (track: Track, minimumRating: number | null, sameGenre: boolean) => void; radioBusy: boolean; radioMessage: string | null; onStopRadio: () => void; radioActive: boolean; }
export function SonicPanel({ track, onPlay, onRadio, radioBusy, radioMessage, onStopRadio, radioActive }: Props) {
  const [response, setResponse] = useState<SonicResponse | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [minimumRating, setMinimumRating] = useState<number | null>(null);
  const [sameGenre, setSameGenre] = useState(false);
  const serial = useRef(0);
  const selection = useRef(track.trackKey);
  useLayoutEffect(() => { selection.current = track.trackKey; }, [track.trackKey]);
  useEffect(() => () => { serial.current++; }, []);
  async function load(analyze = false) {
    const key = track.trackKey, id = ++serial.current; setBusy(true); setError(null);
    try {
      if (analyze) await analyzeSonicSeed(key);
      const result = await sonicMatches({ seedKey: key, excludeKeys: [], limit: 20, minimumRating, sameGenre, radio: false });
      if (id === serial.current && selection.current === key) setResponse(result);
    } catch (failure) { if (id === serial.current && selection.current === key) setError(failure instanceof Error ? failure.message : String(failure)); }
    finally { if (id === serial.current && selection.current === key) setBusy(false); }
  }
  return <section className="sonic-panel" aria-label="More like this">
    <h3>More like this</h3><p>Find music in your library by its sound.</p>
    <div className="sonic-filters"><label>Minimum rating <select value={minimumRating ?? ""} disabled={busy} onChange={e => { setMinimumRating(e.target.value ? Number(e.target.value) : null); setResponse(null); }}><option value="">Any</option><option value="3">3 stars</option><option value="4">4 stars</option><option value="5">5 stars</option></select></label><label><input type="checkbox" checked={sameGenre} disabled={busy} onChange={e => { setSameGenre(e.target.checked); setResponse(null); }} />Same genre</label></div>
    <div className="sonic-actions"><button type="button" disabled={busy} onClick={() => void load()}>{busy ? "Working…" : "Find similar"}</button><button type="button" disabled={radioBusy || busy} onClick={() => onRadio(track, minimumRating, sameGenre)}>Start sonic radio</button>{radioActive && <button type="button" onClick={onStopRadio}>Stop radio</button>}</div>
    {response && <p>{response.analyzed.toLocaleString()} of {response.total.toLocaleString()} tracks analyzed. Matches cover analyzed music.</p>}
    {response && !response.seedReady && <div><p>This track needs audio analysis.</p><button type="button" disabled={busy} onClick={() => void load(true)}>Analyze this track</button><p>Analyze more music in Music Library → Tools → Audio analysis.</p></div>}
    {response?.seedReady && !response.tracks.length && <p>No eligible matches yet. Analyze more music or change the filters.</p>}
    <details><summary>Reuse analysis from another computer</summary><p>In Music Library → Tools → Audio analysis → Backup and cross-PC reuse, back up completed results to OneDrive’s _musicbackup/sonic-analysis folder. On this computer, merge the backup and run Verify reused analysis. Aurora can use each verified track immediately for sonic:yes, similarity, radio and journeys.</p></details>
    {response?.tracks.map(candidate => <button className="sonic-match" type="button" key={candidate.trackKey} onClick={() => onPlay(candidate, response.tracks)}><strong>{candidate.title}</strong><span>{displayTrackArtist(candidate)} · {candidate.album}</span></button>)}
    {error && <p role="alert">{error}</p>}{radioMessage && <p role="status">{radioMessage}</p>}
  </section>;
}
