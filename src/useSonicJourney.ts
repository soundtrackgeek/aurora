import { useEffect, useRef, useState } from "react";
import { buildSonicJourney, searchJourneyTracks, saveSonicJourney, type JourneyRequest, type JourneyResponse, type JourneyTrack } from "./sonicJourney";
export interface JourneyChoice { trackKey: string; title: string; artist: string; }
export function useSonicJourney(onSaved?: () => void) {
  const [stops, setStops] = useState<JourneyChoice[]>([]);
  const [connecting, setConnecting] = useState(3);
  const [rating, setRating] = useState<number | null>(null);
  const [sameGenre, setSameGenre] = useState(false);
  const [text, setText] = useState("");
  const [searched, setSearched] = useState(false);
  const [choices, setChoices] = useState<JourneyTrack[]>([]);
  const [result, setResult] = useState<JourneyResponse | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState("Sonic journey");
  const serial = useRef(0);
  useEffect(() => { const guard = serial; return () => { guard.current++; }; }, []);
  function invalidate() { serial.current++; setResult(null); setMessage(null); setError(null); setBusy(false); }
  function add(track: JourneyChoice) { if (stops.length >= 10 || stops.some(s => s.trackKey === track.trackKey)) return; invalidate(); setStops([...stops, track]); }
  function remove(index: number) { invalidate(); setStops(stops.filter((_, i) => i !== index)); }
  function move(index: number, delta: number) { const next = [...stops]; const target = index + delta; if (target < 0 || target >= next.length) return; [next[index], next[target]] = [next[target], next[index]]; invalidate(); setStops(next); }
  const request: JourneyRequest = { stopKeys: stops.map(s => s.trackKey), connectingTracks: connecting, minimumRating: rating, sameGenre };
  async function run(action: () => Promise<void>) { const id = ++serial.current; setBusy(true); setError(null); setMessage(null); try { await action(); } catch (e) { if (id === serial.current) setError(e instanceof Error ? e.message : String(e)); } finally { if (id === serial.current) setBusy(false); } }
  async function search() { const id = serial.current + 1; await run(async () => { const next = await searchJourneyTracks(text); if (id === serial.current) { setChoices(next); setSearched(true); } }); }
  async function build() { const id = serial.current + 1; setResult(null); await run(async () => { const next = await buildSonicJourney(request); if (id === serial.current) setResult(next); }); }
  async function save() { if (!result?.complete) return; const id = serial.current + 1; await run(async () => { const next = await saveSonicJourney(request, result.tracks, name); if (id === serial.current) { setMessage(next); onSaved?.(); } }); }
  return { stops, connecting, rating, sameGenre, text, choices, searched, result, busy, message, error, name, setName, add, remove, move, search, build, save, run,
    changeConnecting: (value: number) => { invalidate(); setConnecting(value); }, changeRating: (value: number | null) => { invalidate(); setRating(value); }, changeGenre: (value: boolean) => { invalidate(); setSameGenre(value); }, changeText: (value: string) => { serial.current++; setBusy(false); setText(value); setChoices([]); setSearched(false); } };
}
