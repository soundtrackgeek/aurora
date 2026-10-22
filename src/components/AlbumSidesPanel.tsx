import { useEffect, useRef, useState } from "react";
import { exploreAlbumSound, type AlbumSidesResponse, type SideTrack, type SonicAlbum } from "../sonic";
import { isTauriRuntime } from "../library";
import "./AlbumSidesPanel.css";

interface Props {
  albumId: string;
  onOpenAlbum: (album: SonicAlbum) => void;
  onPlay: (tracks: SideTrack[]) => Promise<void>;
}
export function AlbumSidesPanel(props: Props) { return <AlbumSides key={props.albumId} {...props} />; }
function AlbumSides({ albumId, onOpenAlbum, onPlay }: Props) {
  const [result, setResult] = useState<AlbumSidesResponse | null>(null);
  const [busy, setBusy] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const serial = useRef(0);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; const requests = serial; return () => { mounted.current = false; requests.current++; }; }, []);
  async function explore(key: string | null = null) {
    const id = ++serial.current;
    setBusy(true); setError(null);
    setResult(old => key && old ? { ...old, selectedKey: key, matches: [] } : null);
    try { const next = await exploreAlbumSound(albumId, key); if (id === serial.current) setResult(next); }
    catch (e) { if (id === serial.current) setError(e instanceof Error ? e.message : String(e)); }
    finally { if (id === serial.current) setBusy(false); }
  }
  async function play(tracks: SideTrack[]) {
    setPlaying(true); setError(null);
    try { await onPlay(tracks); }
    catch (e) { if (mounted.current) setError(e instanceof Error ? e.message : String(e)); }
    finally { if (mounted.current) setPlaying(false); }
  }
  const selected = result?.sides.flatMap(s => s.tracks).find(t => t.trackKey === result.selectedKey);
  const trackActions = (track: SideTrack) => <div className="album-sides-actions">
    <button type="button" disabled={playing} onClick={() => void play([track])} aria-label={`Play ${track.title}`}>Play track</button>
    <button type="button" disabled={busy} onClick={() => void explore(track.trackKey)} aria-label={`Find albums containing the sound of ${track.title}`}>Find this sound on other albums</button>
  </div>;
  return <section className="sonic-panel album-sides-panel" aria-label="An album’s many sides">
    <div className="album-sides-heading"><span className="album-sides-eyebrow">An album’s many sides</span><h3>Explore its sound</h3></div>
    <p>Find a way into this album through complete tracks and the sounds they share.</p>
    {!isTauriRuntime() && <p>Browser demonstration · illustrative groups and matches.</p>}
    <button type="button" disabled={busy} onClick={() => void explore()}>{busy ? "Exploring…" : result ? "Explore again" : "Explore its sound"}</button>
    {result && <p role="status">{result.analyzedTracks}/{result.totalTracks} tracks have usable current analysis.{!result.ready ? " Complete analysis is needed. Analyze this album in Music Library, then explore again. Banned, missing, changed or unsupported files cannot contribute." : result.sides.length === 1 ? " One representative for this album." : ` ${result.sides.length} sound groups, each represented by a real track.`}</p>}
    {result?.ready && <div className="album-sides-groups">{result.sides.map((side, i) => <article className="album-sides-group" key={side.representative.trackKey}>
      <span className="album-sides-eyebrow">{result.sides.length === 1 ? "A way in" : `Side ${i + 1}`} · {side.tracks.length} {side.tracks.length === 1 ? "track" : "tracks"}</span>
      <h4>{side.representative.title}</h4><p>{side.representative.artist}</p>
      {trackActions(side.representative)}
      {side.tracks.length > 1 && <><button type="button" disabled={playing} onClick={() => void play(side.tracks)}>Play this side</button>
        <details><summary>Tracks close to this one ({side.tracks.length})</summary><ul>{side.tracks.map(t => <li key={t.trackKey}><span>{t.title}</span><button type="button" disabled={playing} onClick={() => void play([t])} aria-label={`Play group track ${t.title}`}>Play</button></li>)}</ul></details></>}
    </article>)}</div>}
    {result?.contrastingTrack && <article className="album-sides-contrast"><span className="album-sides-eyebrow">The strongest contrast</span><h4>{result.contrastingTrack.title}</h4><p>Furthest in sound from the album’s central representative. A different entry point, never a quality judgment.</p>{trackActions(result.contrastingTrack)}</article>}
    {selected && <div className="album-sides-results" aria-busy={busy}><h4>Albums containing the sound of “{selected.title}”</h4>
      <p>Matched by a complete track, so a varied album’s average cannot hide it. Results require complete usable analysis and exclude identical audio to the selected track.</p>
      {!busy && !error && result?.matches.length === 0 && <p>No eligible albums found in this neighborhood. Try another track or analyze more albums.</p>}
      {result?.matches.map(match => <article className="album-sides-match" key={match.album.albumId}><button type="button" className="sonic-match" onClick={() => onOpenAlbum(match.album)}><strong>{match.album.title}</strong><span>{match.album.albumArtist}</span><span>{match.album.analyzedTracks}/{match.album.totalTracks} tracks analyzed · Complete analysis</span></button><p>Way in: {match.matchingTrack.title} · {match.matchingTrack.artist}</p><button type="button" disabled={playing} onClick={() => void play([match.matchingTrack])} aria-label={`Play matching track ${match.matchingTrack.title}`}>Hear the matching track</button></article>)}
      {result?.searchLimited && <p>Search covers a bounded sound neighborhood and checks up to 120 candidate albums; this is a shortlist, not an exhaustive ranking.</p>}
    </div>}
    {error && <p role="alert">{error}</p>}
    <p className="album-sides-note">Whole-track acoustic groups · no mood, instrument or within-track section labels.</p>
  </section>;
}
