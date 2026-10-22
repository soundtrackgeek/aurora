import { useEffect, useRef, useState } from "react";
import { operateHiddenFavorites, type HiddenShelf, type SonicTrack } from "../hiddenFavorites";
import "./HiddenFavorites.css";

export function HiddenFavorites({ onOpen, onPlay, onSaved }: { onOpen: (track: SonicTrack) => void; onPlay?: (track: SonicTrack) => Promise<void>; onSaved?: () => void }) {
  const [shelf, setShelf] = useState<HiddenShelf | null>(null);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const serial = useRef(0);
  useEffect(() => {
    const control = serial;
    const id = ++control.current;
    operateHiddenFavorites("load").then(result => { if (id === serial.current) setShelf(result); }).catch(e => { if (id === serial.current) setError(String(e)); }).finally(() => { if (id === serial.current) setBusy(false); });
    return () => { control.current++; };
  }, []);
  async function run(work: () => Promise<void>) {
    const id = ++serial.current;
    setBusy(true); setError(null); setMessage(null);
    try { await work(); } catch (e) { if (id === serial.current) setError(e instanceof Error ? e.message : String(e)); }
    finally { if (id === serial.current) setBusy(false); }
  }
  async function act(action: string, trackKey: string | null = null) {
    const id = serial.current + 1;
    await run(async () => {
      const result = await operateHiddenFavorites(action, shelf?.revision ?? null, trackKey, action === "save" ? shelf?.kept.map(t => t.trackKey) ?? [] : null);
      if (serial.current === id) { if (action === "save") onSaved?.(); setShelf(result); setMessage(result.message ?? (action === "keep" ? "Kept for later in both apps." : action === "dismiss" ? "Hidden for today. Your rating is unchanged." : null)); }
    });
  }
  const history = shelf?.historyPlays ? `Available history: ${shelf.historyPlays.toLocaleString()} plays since ${shelf.historySince ? new Date(shelf.historySince * 1000).toLocaleDateString() : "the first recorded play"}. No matched play in the last ${shelf.excludedDays} days.` : "No listening history is available. Suggestions use your ratings and saved sound analysis.";
  function play(track: SonicTrack) { if (onPlay) void run(async () => { await onPlay(track); }); }
  return <section className="hidden-favorites" aria-labelledby="hidden-favorites-title" aria-busy={busy}>
    <header className="hidden-favorites__heading"><div><span className="hidden-favorites__eyebrow">Hidden favorites</span><h2 id="hidden-favorites-title">You already own this.</h2><p>Five reasons to look through your own shelves.</p></div><div className="hidden-favorites__actions"><button type="button" disabled={busy} onClick={() => void act("load")}>Reload shelf</button><button type="button" disabled={busy} onClick={() => void act("refresh")}>{busy ? "Working…" : shelf?.generatedAt ? "Find five more" : "Find hidden favorites"}</button></div></header>
    {shelf && <><p className="hidden-favorites__note">{shelf.note}</p><p className="hidden-favorites__history">{history}</p>
      {shelf.generatedAt && <small>Chosen {new Date(shelf.generatedAt).toLocaleString()}. The shelf changes when you ask; newly played or unavailable tracks drop out.</small>}
      {shelf.modelAuc !== null && <details className="hidden-favorites__model"><summary>About the taste evidence</summary><p>{shelf.modelVersion} · {shelf.trainingTracks.toLocaleString()} training tracks · {shelf.validationTracks.toLocaleString()} tracks by held-out album artists · AUC {shelf.modelAuc.toFixed(3)}. All music families pooled; this is not a probability that you will like a track.</p></details>}
      <div className="hidden-favorites__grid">{shelf.suggestions.map(item => <article className={`hidden-favorites__card${item.broader ? " hidden-favorites__card--broader" : ""}`} key={item.track.trackKey} aria-label={`${item.track.title} by ${item.track.artist}`}><span className="hidden-favorites__fit">{item.broader ? "A broader discovery" : item.fit}{item.track.rating !== null ? ` · ${item.track.rating / 20}★` : " · Unrated"}</span><button type="button" className="hidden-favorites__title" aria-label={`Open ${item.track.title} by ${item.track.artist}`} onClick={() => onOpen(item.track)}><strong>{item.track.title}</strong><span>{item.track.artist}</span></button><p className="hidden-favorites__album">{item.track.album}</p><p className="hidden-favorites__anchors">Sound neighbors of {item.anchors.map((a, i) => <span key={a.trackKey}>{i > 0 ? " and " : ""}<button type="button" onClick={() => onOpen(a)}>{a.title} — {a.artist}</button></span>)}</p><div className="hidden-favorites__actions">{onPlay && <button type="button" disabled={busy} onClick={() => play(item.track)}>Play</button>}<button type="button" disabled={busy || item.kept} onClick={() => void act("keep", item.track.trackKey)}>{item.kept ? "Kept for later" : "Keep for later"}</button><button type="button" disabled={busy} onClick={() => void act("dismiss", item.track.trackKey)}>Not today</button></div></article>)}</div>
      {shelf.generatedAt && shelf.suggestions.length === 0 && <p>No eligible suggestions on this shelf. Find five more after analyzing more music, or reload to see changes from the other app.</p>}
      <details className="hidden-favorites__kept"><summary>Kept for later ({shelf.kept.length})</summary>{shelf.kept.length > 0 ? <><ul>{shelf.kept.map(track => <li key={track.trackKey}><button type="button" onClick={() => onOpen(track)}>{track.title} — {track.artist}</button><div>{onPlay && <button type="button" disabled={busy} onClick={() => play(track)}>Play</button>}<button type="button" disabled={busy} aria-label={`Remove ${track.title} from kept tracks`} onClick={() => void act("remove", track.trackKey)}>Remove</button></div></li>)}</ul><button type="button" disabled={busy} onClick={() => void act("save")}>Save kept tracks as a playlist</button></> : <p>Keep a few tracks here, then save them together in Playlists.</p>}</details>
    </>}
    {message && <p role="status">{message}</p>}{error && <p role="alert">{error}</p>}
  </section>;
}
