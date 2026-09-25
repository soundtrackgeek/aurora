import { useEffect, useRef, useState } from "react";
import { Check, Moon, Play, RefreshCw, ThumbsUp, X } from "lucide-react";
import { Artwork } from "../Artwork";
import type { RatingAlbum } from "../../ratings";
import { defaultTonightRequest, requestTonight, resetTonightFeedback, saveTonightFeedback, tonightAlbumTrack, type TonightIntention, type TonightRequest, type TonightResult } from "../../tonight";
import "./TonightsAlbum.css";

const storageKey = "aurora.tonight.v1";
const intentions: { value: TonightIntention; label: string; detail: string }[] = [
  { value: "comfort", label: "Comfort", detail: "A familiar favorite" },
  { value: "discovery", label: "Discovery", detail: "Something less explored" },
  { value: "finish", label: "Finish something", detail: "Complete an album’s ratings" },
];
function readSaved(): { draft: TonightRequest; result: TonightResult | null } {
  try {
    const saved = JSON.parse(localStorage.getItem(storageKey) ?? "null");
    const draft = saved?.draft;
    if (!intentions.some(item => item.value === draft?.intention) || !Number.isInteger(draft.minutes) || draft.minutes < 10 || draft.minutes > 240 || typeof draft.description !== "string" || draft.description.length > 300 || typeof draft.useJev !== "boolean") throw new Error("Invalid saved choices");
    const result = saved.result;
    if (result && (!Array.isArray(result.suggestions) || result.suggestions.length > 3 || !result.request || !Number.isFinite(result.generatedAtMs) || typeof result.message !== "string" || !result.suggestions.every((item: TonightResult["suggestions"][number]) => item.album && typeof item.album.id === "string" && typeof item.album.title === "string" && typeof item.album.artist === "string" && Array.isArray(item.reasons) && item.reasons.every(reason => typeof reason === "string")))) throw new Error("Invalid saved suggestions");
    return { draft: { ...draft, excludeIds: [] }, result };
  } catch { return { draft: { ...defaultTonightRequest }, result: null }; }
}
function sameChoices(a: TonightRequest, b: TonightRequest): boolean {
  return a.intention === b.intention && a.minutes === b.minutes && a.description === b.description && a.useJev === b.useJev;
}

export function TonightsAlbum({ onPlay, onOpen, onSettings, playbackBusy = false }: {
  onPlay: (album: RatingAlbum, minutes: number) => Promise<void>;
  onOpen: (album: RatingAlbum) => void;
  onSettings: () => void;
  playbackBusy?: boolean;
}) {
  const [initial] = useState(readSaved);
  const [draft, setDraft] = useState(initial.draft);
  const [result, setResult] = useState(initial.result);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const operation = useRef(false);
  const [expanded, setExpanded] = useState(true);
  useEffect(() => {
    try { localStorage.setItem(storageKey, JSON.stringify({ draft, result })); } catch { /* Current choices still work if storage is unavailable. */ }
  }, [draft, result]);
  const changed = result !== null && !sameChoices(draft, result.request);
  const locked = busy || playbackBusy;

  async function run(action: () => Promise<void>) {
    if (operation.current || playbackBusy) return;
    operation.current = true; setBusy(true); setMessage("");
    try { await action(); } catch (error) { setMessage(String(error instanceof Error ? error.message : error)); }
    finally { operation.current = false; setBusy(false); }
  }
  async function suggest(none = false) {
    await run(async () => {
      if (none && result?.suggestions.length) {
        await saveTonightFeedback(result.request.intention, result.suggestions.map(item => item.album.id), "dismiss");
        // Successful feedback remains visible even when the following retrieval fails.
        setResult({ ...result, suggestions: [] });
      }
      const excludeIds = result?.request.intention === draft.intention ? result.suggestions.map(item => item.album.id) : [];
      setResult(await requestTonight({ ...draft, excludeIds }));
      if (none) setMessage("Remembered: those albums don’t fit this intention.");
    });
  }
  async function feedback(id: string, value: "good" | "dismiss") {
    if (!result) return;
    await run(async () => {
      await saveTonightFeedback(result.request.intention, [id], value);
      setResult({ ...result, suggestions: value === "dismiss" ? result.suggestions.filter(item => item.album.id !== id) : result.suggestions.map(item => item.album.id === id ? { ...item, feedback: "good" } : item) });
      setMessage(value === "good" ? "Good fit remembered for this intention." : "Dismissed for this intention. Request more when you’re ready.");
    });
  }

  return <section className="tonight" aria-labelledby="tonight-title" aria-busy={busy}>
    <header className="tonight__header"><button type="button" className="tonight__toggle" aria-expanded={expanded} aria-controls="tonight-content" onClick={() => setExpanded(!expanded)}><Moon aria-hidden="true" /><span><strong id="tonight-title">Tonight’s Album</strong><small>A little less choosing. A little more listening.</small></span></button><span className="tonight__badge">{result?.source === "jev" ? "Jev + your library" : result?.source === "preview" ? "Sample library" : "Your library"}</span></header>
    <div id="tonight-content" hidden={!expanded}>
      <div className="tonight__controls">
        <div className="tonight__intentions" role="group" aria-label="Listening intention">{intentions.map(item => <button key={item.value} type="button" aria-pressed={draft.intention === item.value} disabled={locked} onClick={() => setDraft({ ...draft, intention: item.value })}><strong>{item.label}</strong><small>{item.detail}</small></button>)}</div>
        <label className="tonight__time">Time to listen<select aria-label="Time to listen" value={draft.minutes} disabled={locked} onChange={event => setDraft({ ...draft, minutes: Number(event.target.value) })}>{[20, 30, 45, 60, 90, 120, 180, 240].map(value => <option key={value} value={value}>{value} minutes</option>)}</select></label>
      </div>
      <div className="tonight__brief"><label>What would fit? <span>Optional</span><input maxLength={300} placeholder="For example, melodic rock or quiet electronic music" value={draft.description} disabled={locked} onChange={event => setDraft({ ...draft, description: event.target.value })} /></label><button className="button button--primary" type="button" disabled={locked} onClick={() => void suggest()}><RefreshCw aria-hidden="true" className={busy ? "is-spinning" : undefined} />{busy ? "Choosing…" : result ? "Suggest three more" : "Find three albums"}</button></div>
      <div className="tonight__options"><label><input type="checkbox" checked={draft.useJev} disabled={locked} onChange={event => setDraft({ ...draft, useJev: event.target.checked })} /> Ask Jev to help</label><button type="button" onClick={onSettings}>Jev settings</button><button type="button" disabled={locked} onClick={() => void run(async () => { await resetTonightFeedback(); setResult(null); setMessage("Album feedback reset. Request suggestions when you’re ready."); })}>Reset album feedback</button></div>
      {!result && <p className="tonight__note">Three whole albums that fit your time. Suggestions change only when you ask. Jev uses a small metadata shortlist; local ranking is always available.</p>}
      {result && <><div className="tonight__result-heading"><p>For <strong>{intentions.find(item => item.value === result.request.intention)?.label.toLowerCase()}</strong> · {result.request.minutes} minutes{result.request.description ? ` · “${result.request.description}”` : ""}</p><small>Chosen {new Date(result.generatedAtMs).toLocaleString()}</small></div>{changed && <p className="tonight__note">Your choices changed. Request suggestions to apply them.</p>}<p className="tonight__note">{result.message}</p>
        <div className="tonight__cards">{result.suggestions.map(item => <article className="tonight__card" key={item.album.id} aria-label={`${item.album.title} by ${item.album.artist}`}>
          <button type="button" className="tonight__cover" onClick={() => onOpen(item.album)} aria-label={`Open ${item.album.title} in library`}><Artwork track={tonightAlbumTrack(item.album)} size="large" /></button>
          <div className="tonight__card-body"><h3>{item.album.title}</h3><p>{item.album.artist}</p><small>{[item.album.originalYear, item.album.genre].filter(Boolean).join(" · ")}</small><ul>{item.reasons.map(reason => <li key={reason}>{reason}</li>)}</ul>
          <button type="button" className="button button--primary tonight__play" disabled={locked} onClick={() => void run(async () => { await onPlay(item.album, result.request.minutes); setMessage(`Playing ${item.album.title}.`); })}><Play aria-hidden="true" /> Play album</button>
          <div className="tonight__feedback"><button type="button" aria-pressed={item.feedback === "good"} disabled={locked || item.feedback === "good"} onClick={() => void feedback(item.album.id, "good")}>{item.feedback === "good" ? <Check aria-hidden="true" /> : <ThumbsUp aria-hidden="true" />} Good fit</button><button type="button" disabled={locked} onClick={() => void feedback(item.album.id, "dismiss")}><X aria-hidden="true" /> Not for this</button></div></div>
        </article>)}</div>
        {result.suggestions.length === 0 ? <p className="tonight__empty">No suggestions to show. Try more time, another intention, or reset album feedback.</p> : <button type="button" className="button tonight__none" disabled={locked || changed} onClick={() => void suggest(true)}>None of these — find another three</button>}
      </>}
      {message && <p className="tonight__message" role="status">{message}</p>}
    </div>
  </section>;
}
