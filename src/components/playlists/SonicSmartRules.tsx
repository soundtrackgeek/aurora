import { useRef, useState } from "react";
import { searchJourneyTracks } from "../../sonicJourney";
import type { JourneyTrack } from "../../sonicJourney";
import type { SonicPlaylistRecipe } from "../../playlists";

export function SonicSmartRules({ value, onChange, disabled }: { value: SonicPlaylistRecipe | null; onChange: (value: SonicPlaylistRecipe | null) => void; disabled: boolean }) {
  const [text, setText] = useState("");
  const [choices, setChoices] = useState<JourneyTrack[]>([]);
  const [searching, setSearching] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const serial = useRef(0);
  async function search() {
    const id = ++serial.current;
    setSearching(true); setMessage(null); setChoices([]);
    try {
      const tracks = await searchJourneyTracks(text);
      if (id === serial.current) { setChoices(tracks); if (!tracks.length) setMessage("No current analyzed songs found. Try another artist or title, or run Audio analysis."); }
    } catch (reason) { if (id === serial.current) setMessage(String(reason)); }
    finally { if (id === serial.current) setSearching(false); }
  }
  return <section className="sonic-smart-rules" aria-label="Sonic Smart rules">
    <label><input type="checkbox" checked={value !== null} disabled={disabled} onChange={e => { serial.current++; setSearching(false); setChoices([]); setMessage(null); onChange(e.target.checked ? { version: 1, profile: "bliss-0.13.0-symphonia-0.6.1-v2-full-mp3", seeds: [], maxDistance: 0.2, maxPerArtist: 2, ordering: "nearest" } : null); }} />Sounds like these examples</label>
    {value && <>
      <p>Choose 1–8 analyzed songs. Matches are nearest to any example, within the distance below, and must pass all your metadata rules. Examples are excluded. Refresh updates the saved playlist and keeps playing queues stable.</p>
      <label>Find sound examples<input maxLength={200} value={text} disabled={disabled} placeholder="Artist or song title" onKeyDown={e=>{if(e.key==="Enter"){e.preventDefault();if(!disabled&&!searching&&text.trim().length>=2)void search();}}} onChange={e => { serial.current++; setSearching(false); setText(e.target.value); setChoices([]); setMessage(null); }} /></label>
      <button className="secondary-button" type="button" disabled={disabled || searching || text.trim().length < 2} onClick={() => void search()}>{searching ? "Searching…" : "Search examples"}</button>
      {choices.length > 0 && <ul>{choices.map(t => <li key={t.trackKey}><span>{t.title} · {t.artist} · {t.album}</span><button className="secondary-button" type="button" disabled={disabled || value.seeds.length >= 8 || value.seeds.some(s => s.trackKey === t.trackKey)} onClick={() => onChange({ ...value, seeds: [...value.seeds, { trackKey: t.trackKey, title: t.title, artist: t.artist, audioHash: null }] })}>Add {t.title}</button></li>)}</ul>}
      <ol aria-label="Sound examples">{value.seeds.map((s, i) => <li key={s.trackKey}><span>{s.title} · {s.artist}</span><button className="secondary-button" type="button" disabled={disabled} onClick={() => onChange({ ...value, seeds: value.seeds.filter((_, n) => n !== i) })}>Remove {s.title}</button></li>)}</ol>
      {!value.seeds.length && <p role="status">Add a sound example before saving.</p>}
      <label>Maximum acoustic distance<input type="number" min={0.001} max={2} step={0.001} required disabled={disabled} value={value.maxDistance} onChange={e => onChange({ ...value, maxDistance: Number(e.target.value) })} /></label>
      <p>Lower means closer in whole-track sound. Start at 0.2; this is a distance, not a percentage, mood or BPM rule.</p>
      <label>Maximum songs per track artist<input type="number" min={1} max={100} required disabled={disabled} value={value.maxPerArtist} onChange={e => onChange({ ...value, maxPerArtist: Number(e.target.value) })} /></label>
      <p>Order: nearest example first, with stable file-identity ties. Missing examples stop refresh; too few matches keep your rules and show a smaller playlist.</p>
      {message && <p role="status">{message}</p>}
    </>}
  </section>;
}
