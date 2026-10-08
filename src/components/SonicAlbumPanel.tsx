import { useEffect, useRef, useState } from "react";
import { sonicAlbumMatches, type SonicAlbum, type SonicAlbumMatches } from "../sonic";
import "./SonicPanel.css";

interface Props {
  albumId: string;
  onOpenAlbum: (album: SonicAlbum) => void;
  onPlayAlbum: (album: SonicAlbum) => Promise<void>;
  onRadio: (album: SonicAlbum, rating: number | null, sameGenre: boolean, coverage: number) => void;
  radioBusy: boolean;
  radioMessage: string | null;
  radioActive: boolean;
  onStopRadio: () => void;
}
export function SonicAlbumPanel(props: Props) { return <AlbumSimilarity key={props.albumId} {...props} />; }
function AlbumSimilarity({ albumId, onOpenAlbum, onPlayAlbum, onRadio, radioBusy, radioMessage, radioActive, onStopRadio }: Props) {
  const [coverage, setCoverage] = useState(50);
  const [rating, setRating] = useState<number | null>(null);
  const [sameGenre, setSameGenre] = useState(false);
  const [result, setResult] = useState<SonicAlbumMatches | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const serial = useRef(0);
  useEffect(() => { const control = serial; return () => { control.current++; }; }, []);
  async function find() {
    const id = ++serial.current; setBusy(true); setError(null); setResult(null);
    try { const next = await sonicAlbumMatches(albumId, coverage); if (id === serial.current) setResult(next); }
    catch (e) { if (id === serial.current) setError(e instanceof Error ? e.message : String(e)); }
    finally { if (id === serial.current) setBusy(false); }
  }
  async function play(album: SonicAlbum) {
    const id = ++serial.current; setBusy(true); setError(null);
    try { await onPlayAlbum(album); }
    catch (e) { if (id === serial.current) setError(e instanceof Error ? e.message : String(e)); }
    finally { if (id === serial.current) setBusy(false); }
  }
  return <section className="sonic-panel sonic-album-panel" aria-label="Similar sounding albums">
    <h3>More like this album</h3>
    <p>Compare the sound of analyzed tracks. Partial albums need three usable tracks, or all tracks for shorter albums.</p>
    <div className="sonic-filters"><label className="sonic-coverage">Minimum analysis coverage<select value={coverage} disabled={busy || radioBusy} onChange={e => { setCoverage(Number(e.target.value)); setResult(null); }}>
      <option value="50">50% or more</option><option value="80">80% or more</option><option value="100">Complete albums only</option>
    </select></label></div>
    <div className="sonic-actions"><button type="button" disabled={busy} onClick={() => void find()}>{busy ? "Working…" : "Find similar albums"}</button></div>
    {result && <p role="status">{result.seed ? `${result.seed.analyzedTracks}/${result.seed.totalTracks} MP3 tracks usable in this album.` : "This album is no longer in the library."} {result.seedReady ? `Comparing ${result.analyzedAlbums.toLocaleString()} albums with enough saved analysis.` : "Analyze this album in Music Library → Albums, then find again."}</p>}
    {result?.seedReady && !result.albums.length && <p>No eligible albums yet. Analyze more albums or lower the coverage setting.</p>}
    {result?.albums.map(album => <div className="sonic-album-match" key={album.albumId}>
      <button className="sonic-match" type="button" onClick={() => onOpenAlbum(album)}><strong>{album.title}</strong><span>{album.albumArtist}</span><span>{album.analyzedTracks}/{album.totalTracks} tracks analyzed{album.analyzedTracks < album.totalTracks ? " · Partial analysis" : " · Complete analysis"}</span></button>
      <button type="button" disabled={busy} onClick={() => void play(album)} aria-label={`Play ${album.title}`}>Play album</button>
    </div>)}
    {result?.seedReady && result.seed && <><div className="sonic-filters"><label>Radio minimum rating<select value={rating ?? ""} onChange={e => setRating(e.target.value ? Number(e.target.value) : null)}><option value="">Any</option><option value="3">3 stars</option><option value="4">4 stars</option><option value="5">5 stars</option></select></label><label><input type="checkbox" checked={sameGenre} onChange={e => setSameGenre(e.target.checked)} />Radio same genre</label></div>
      <div className="sonic-actions"><button type="button" disabled={busy || radioBusy} onClick={() => onRadio(result.seed!, rating, sameGenre, coverage)}>Start album sonic radio</button></div></>}
    {radioActive && <button type="button" onClick={onStopRadio}>Stop radio</button>}
    {radioMessage && <p role="status">{radioMessage}</p>}{error && <p role="alert">{error}</p>}
  </section>;
}
